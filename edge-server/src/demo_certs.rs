//! Demo certificates for the examples, like a certificate service that finds
//! its files missing.
//!
//! Before the certificate provider and TLS resolver read the application's
//! `tls.yaml`, this reads it too and makes whatever it names that is not there
//! yet: the CA, the default identity and one certificate per host, through
//! Claviron's own issuing path. Keys are made on the machine that runs the
//! example and are never committed. Present files are kept, so a CA already
//! trusted in a browser stays trusted; delete the store to start over.
//!
//! ```ignore
//! Server::with_options(options).with_services::<DemoCertificates>()
//! ```

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use claviron::registry::{self, Bootable, Component, ComponentOrder};
use claviron::ssl::CertificateKind;
use claviron::ssl::certstore::{
    CertEntry, CertStore, FULLCHAIN_CERT_FILENAME, PRIVKEY_FILENAME,
};
use claviron::ssl::formatters::certificate_chain_to_pem;
use claviron::ssl::generators::{generate_ca_cert, generate_ca_key};
use claviron::ssl::transfer::private_key_to_pem;
use claviron::tls::config::{TlsConfig, TlsConfigSource};
use claviron::tls::{CertificateProvider, CertificateStore};
use claviron::{AppState, ConfigReader, Register};

const AUTHORITY_NAME: &str = "Claviron Examples Demo CA";
const AUTHORITY_DAYS: u32 = 3650;
/// Browsers refuse a server certificate that lasts longer than 398 days.
const SERVER_DAYS: u32 = 398;

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// Makes what `tls.yaml` names at boot, before TLS reads it.
pub struct DemoCertificates;

impl Register<AppState> for DemoCertificates {
    fn register(state: &AppState) -> registry::Result<()> {
        state.registry_ref().register(Arc::new(Self))?;
        Ok(())
    }
}

impl Component<AppState> for DemoCertificates {
    type Deps = ();

    fn name(&self) -> &'static str {
        "DemoCertificates"
    }

    fn order(&self) -> ComponentOrder {
        ComponentOrder::new().before::<CertificateProvider>()
    }

    fn as_bootable(&self) -> Option<&dyn Bootable<AppState, Self::Deps>> {
        Some(self)
    }
}

#[async_trait]
impl Bootable<AppState> for DemoCertificates {
    async fn boot(
        &self,
        _deps: (),
        state: &AppState,
    ) -> registry::Result<()> {
        ensure(&state.config_dir().join("tls.yaml"))
            .await
            .map_err(|error| registry::Error::msg(error.to_string()))
    }
}

async fn ensure(tls: &Path) -> Result<(), Failure> {
    // Without the file this process has no TLS of its own, and needs nothing.
    if !tls.is_file() {
        return Ok(());
    }
    let source: TlsConfigSource = ConfigReader::new()
        .with_builtin_parsers()?
        .read_with_variables(tls, &std::env::vars().collect())
        .await?;
    let config = TlsConfig::try_from(source)?;
    let files = config.certstore().clone();
    let missing = declared(&config)
        .into_iter()
        .filter(|name| {
            let server = files.server_dir(name);
            !server.join(FULLCHAIN_CERT_FILENAME).is_file()
                || !server.join(PRIVKEY_FILENAME).is_file()
        })
        .collect::<Vec<_>>();
    if missing.is_empty() {
        return Ok(());
    }
    ensure_authority(&files).await?;
    let store = CertificateStore::new(files.clone());
    let authority = store
        .authority()
        .await?
        .ok_or("tls.yaml names no certstore authority")?;
    let signer = Arc::new(store.signer(&authority).await?);
    for name in missing {
        let issuing = Arc::clone(&signer);
        let host = name.clone();
        let issued = tokio::task::spawn_blocking(move || {
            issuing.issue_certificate(
                CertificateKind::Server,
                &host.parse()?,
                std::slice::from_ref(&host),
                SERVER_DAYS,
                None,
            )
        })
        .await??;
        let server = files.server_dir(&name);
        let chain = server.join(FULLCHAIN_CERT_FILENAME);
        write(
            &server.join(PRIVKEY_FILENAME),
            &private_key_to_pem(&issued.key_der)?,
            0o600,
        )?;
        write(&chain, &certificate_chain_to_pem(&issued.certs_der)?, 0o644)?;
        tracing::info!(path = %chain.display(), "created a demo certificate");
    }
    Ok(())
}

/// The store folders `tls.yaml` expects. Paths it names outright are the
/// operator's own files, and a bare name that is no folder name is in hosting.
fn declared(config: &TlsConfig) -> Vec<String> {
    let default = config
        .default_cert_entry()
        .basename()
        .map(|name| name.as_str().to_owned());
    let hosts = config.sni_entries().iter().filter_map(|entry| match entry {
        CertEntry::Name(host) => Some(host.to_string()),
        CertEntry::Paths(_) => None,
    });
    default.into_iter().chain(hosts).collect()
}

async fn ensure_authority(files: &CertStore) -> Result<(), Failure> {
    let name = files
        .authority_entry()
        .and_then(|authority| authority.basename())
        .ok_or("tls.yaml must name its certstore authority with `local`")?;
    let (cert_path, key_path) =
        (files.ca_cert_path(name), files.ca_key_path(name));
    if cert_path.is_file() && key_path.is_file() {
        return Ok(());
    }
    let organization = files.organization().clone();
    let (cert, key) = tokio::task::spawn_blocking(move || {
        let key = generate_ca_key()?;
        let cert = generate_ca_cert(
            &AUTHORITY_NAME.parse()?,
            &key,
            AUTHORITY_DAYS,
            &organization,
        )?;
        Ok::<_, Failure>((cert, key))
    })
    .await??;
    write(&key_path, &key.serialize_pem(), 0o600)?;
    write(&cert_path, &certificate_chain_to_pem(cert.as_ref())?, 0o644)?;
    tracing::info!(
        path = %cert_path.display(),
        "created a demo CA; trust it to open the .test hosts in a browser"
    );
    Ok(())
}

fn write(
    path: &Path,
    content: &str,
    mode: u32,
) -> std::io::Result<()> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory)?;
    }
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(mode)
        .open(path)?
        .write_all(content.as_bytes())
}
