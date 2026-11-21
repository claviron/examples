//! Demo certificates for the examples, like a certificate service that finds
//! its files missing.
//!
//! Before Claviron's certificate provider boots, this reads the TLS
//! declaration the host gave it and makes whatever the declaration names that
//! is not there yet: the default identity and one certificate per host,
//! through Claviron's own issuing path. The declaration is the host's own,
//! read through Claviron rather than parsed again here: an Edge's `tls.yaml`,
//! or the `tls` section a runner's launch file supplies in the same schema.
//! A process without one has no TLS of its own and needs nothing.
//!
//! ```ignore
//! // An Edge, with config/tls.yaml:
//! Server::with_options(options).with_services::<DemoCertificates>()
//! // A Lumenize runner, with a `tls` section in lumenize.yaml:
//! Runner::new().with_options(options).with_services::<DemoCertificates>()
//! ```
//!
//! # Client identities
//!
//! A listener or a host that requires a client certificate needs one to be
//! tried with. The TLS declaration names the store and the certificates its
//! process serves, not the certificates its clients present, so the host
//! names those here instead, the same way in an Edge and in a runner:
//!
//! ```ignore
//! Runner::new().with_bootstrap(|state| {
//!     DemoCertificates::register_with_clients(state, &["client.example.test"])
//! })
//! ```
//!
//! Each is `clients/<name>/fullchain.pem` and `privkey.pem` in the store,
//! issued from the store's authority with that name as its common name. It
//! verifies where `client_auth` selects that authority. Present, it is kept;
//! half present, the boot stops rather than replace the half that is there.
//!
//! # The authority
//!
//! An existing authority is used, never replaced. The store names it with
//! `authority: {local: <name>}`, which is `ca/<name>/ca.crt` and
//! `ca/<name>/ca.key` inside the store. Only a missing identity makes this
//! look at the authority, and then one of these holds:
//!
//! - **Certificate and key present.** Missing identities are issued from it;
//!   nothing about the authority is rewritten. This is how an authority a
//!   browser already trusts is dropped into the store and used.
//! - **Neither present.** A new authority is generated, then the identities.
//!   Its creation is logged once, with the certificate's path: a browser has
//!   to trust it before it opens these hosts.
//! - **Certificate without its key.** Nothing can be issued from it, and no
//!   other authority is generated in its place: the missing identity fails,
//!   saying the key is absent. A key without its certificate fails the same
//!   way.
//!
//! Present identities are kept in every case; delete one to have it issued
//! again. Keys are made on the machine that runs the example and stay in the
//! store, which the examples keep out of Git.
//!
//! # Validity
//!
//! A generated authority lasts 3650 days and an issued identity 398, the
//! longest server certificate a browser accepts; a client identity takes the
//! same. They are fixed here for the examples; an API version should take
//! them from the host's configuration.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use claviron::registry::{self, Bootable, Component, ComponentOrder};
use claviron::ssl::certstore::{
    CertEntry, CertStore, FULLCHAIN_CERT_FILENAME, PRIVKEY_FILENAME,
};
use claviron::ssl::formatters::certificate_chain_to_pem;
use claviron::ssl::generators::{generate_ca_cert, generate_ca_key};
use claviron::ssl::transfer::private_key_to_pem;
use claviron::ssl::{CertificateKind, CommonName};
use claviron::tls::config::{TlsConfig, TlsConfigSource};
use claviron::tls::{CertificateProvider, CertificateStore};
use claviron::{AppState, Register};

const AUTHORITY_NAME: &str = "Claviron Examples Demo CA";
const AUTHORITY_DAYS: u32 = 3650;
/// Browsers refuse a server certificate that lasts longer than 398 days.
/// Client identities take the same.
const SERVER_DAYS: u32 = 398;

type Failure = Box<dyn std::error::Error + Send + Sync>;

/// Makes what the host's TLS declaration names at boot, before TLS reads it,
/// and the client identities the host names.
pub struct DemoCertificates {
    clients: Box<[CommonName]>,
}

impl Register<AppState> for DemoCertificates {
    fn register(state: &AppState) -> registry::Result<()> {
        state
            .registry_ref()
            .register(Arc::new(Self { clients: Box::default() }))?;
        Ok(())
    }
}

impl DemoCertificates {
    /// Register it, also making these client identities when they are
    /// missing: `clients/<name>/` in the store, `name` its common name.
    pub fn register_with_clients(
        state: &AppState,
        clients: &[&str],
    ) -> registry::Result<()> {
        let clients = clients
            .iter()
            .map(|name| {
                name.parse().map_err(|error| {
                    registry::Error::msg(format!(
                        "demo client identity '{name}': {error}"
                    ))
                })
            })
            .collect::<registry::Result<_>>()?;
        state.registry_ref().register(Arc::new(Self { clients }))?;
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
        let declaration = claviron::tls::declaration(state)
            .map_err(|error| registry::Error::msg(error.to_string()))?;
        // Without a declaration this process has no TLS of its own.
        let Some(declaration) = declaration else {
            if self.clients.is_empty() {
                return Ok(());
            }
            return Err(registry::Error::msg(
                "demo client identities need the host's TLS declaration: it names the store and the authority that signs them",
            ));
        };
        ensure(TlsConfigSource::clone(&declaration), &self.clients)
            .await
            .map_err(|error| registry::Error::msg(error.to_string()))
    }
}

async fn ensure(
    source: TlsConfigSource,
    clients: &[CommonName],
) -> Result<(), Failure> {
    // Resolving loads a certstore document the declaration may name: file IO.
    let config =
        tokio::task::spawn_blocking(move || TlsConfig::try_from(source))
            .await??;
    let files = config.certstore().clone();
    let missing = declared(&config)
        .into_iter()
        .filter(|name| {
            let server = files.server_dir(name);
            !server.join(FULLCHAIN_CERT_FILENAME).is_file()
                || !server.join(PRIVKEY_FILENAME).is_file()
        })
        .collect::<Vec<_>>();
    let mut missing_clients = Vec::new();
    for client in clients {
        let (chain, key) = files
            .identity_paths(CertificateKind::Client, client.as_str())
            .ok_or("a client identity has a store folder")?;
        match (chain.try_exists()?, key.try_exists()?) {
            (true, true) => {}
            (false, false) => missing_clients.push((client, chain, key)),
            (true, false) | (false, true) => {
                return Err(format!(
                    "the demo client identity {client} is half present ({} and {}); \
                     delete what is there to have it issued again",
                    chain.display(),
                    key.display(),
                )
                .into());
            }
        }
    }
    if missing.is_empty() && missing_clients.is_empty() {
        return Ok(());
    }
    let named = missing
        .iter()
        .cloned()
        .chain(missing_clients.iter().map(|(client, ..)| client.to_string()))
        .collect::<Vec<_>>()
        .join(", ");
    let authority = AuthorityFiles::of(&files)?;
    match authority.state()? {
        AuthorityState::Present => {}
        AuthorityState::Absent => authority.generate(&files).await?,
        AuthorityState::CertificateWithoutKey => {
            return Err(format!(
                "cannot issue {}: the authority certificate {} is present but its key {} is absent; \
                 nothing can be issued from it, and no other authority is made in its place",
                named,
                authority.certificate.display(),
                authority.key.display(),
            )
            .into());
        }
        AuthorityState::KeyWithoutCertificate => {
            return Err(format!(
                "cannot issue {}: the authority key {} is present but its certificate {} is absent; \
                 no other authority is made in its place",
                named,
                authority.key.display(),
                authority.certificate.display(),
            )
            .into());
        }
    }
    let store = CertificateStore::new(files.clone());
    let loaded =
        store.authority().await?.ok_or("the certstore names no authority")?;
    let signer = Arc::new(store.signer(&loaded).await?);
    for host in missing {
        let server = files.server_dir(&host);
        let issuing = Arc::clone(&signer);
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
        let chain = server.join(FULLCHAIN_CERT_FILENAME);
        write(
            &server.join(PRIVKEY_FILENAME),
            &private_key_to_pem(&issued.key_der)?,
            0o600,
        )?;
        write(&chain, &certificate_chain_to_pem(&issued.certs_der)?, 0o644)?;
        tracing::info!(path = %chain.display(), "created a demo certificate");
    }
    for (client, chain, key) in missing_clients {
        let issuing = Arc::clone(&signer);
        let name = client.clone();
        let issued = tokio::task::spawn_blocking(move || {
            issuing.issue_client_certificate(&name, &[], None, SERVER_DAYS)
        })
        .await??;
        write(&key, &private_key_to_pem(&issued.key_der)?, 0o600)?;
        write(&chain, &certificate_chain_to_pem(&issued.certs_der)?, 0o644)?;
        tracing::info!(path = %chain.display(), "created a demo client certificate");
    }
    Ok(())
}

/// The store folders the declaration expects. Paths it names outright are
/// the operator's own files, and a bare name that is no folder name is in
/// hosting.
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

/// What the store holds of its authority, before anything is issued.
enum AuthorityState {
    /// Certificate and key: issue from it, rewrite nothing.
    Present,
    /// Neither: generate one.
    Absent,
    /// A certificate whose key is not here: nothing can be issued.
    CertificateWithoutKey,
    /// A key whose certificate is not here: nothing can be issued either.
    KeyWithoutCertificate,
}

struct AuthorityFiles {
    certificate: PathBuf,
    key: PathBuf,
}

impl AuthorityFiles {
    fn of(files: &CertStore) -> Result<Self, Failure> {
        let name = files
            .authority_entry()
            .and_then(|authority| authority.basename())
            .ok_or("the certstore must name its authority with `local`")?;
        Ok(Self {
            certificate: files.ca_cert_path(name),
            key: files.ca_key_path(name),
        })
    }

    fn state(&self) -> std::io::Result<AuthorityState> {
        Ok(match (self.certificate.try_exists()?, self.key.try_exists()?) {
            (true, true) => AuthorityState::Present,
            (false, false) => AuthorityState::Absent,
            (true, false) => AuthorityState::CertificateWithoutKey,
            (false, true) => AuthorityState::KeyWithoutCertificate,
        })
    }

    async fn generate(
        &self,
        files: &CertStore,
    ) -> Result<(), Failure> {
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
        write(&self.key, &key.serialize_pem(), 0o600)?;
        write(
            &self.certificate,
            &certificate_chain_to_pem(cert.as_ref())?,
            0o644,
        )?;
        tracing::info!(
            path = %self.certificate.display(),
            "created a new demo certificate authority; trust it to open these hosts in a browser"
        );
        Ok(())
    }
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
