# Demo database

`demo.sqlite` contains prepared synthetic users, todos and demo records.
Migrations, factories and seeders live in this directory. The application uses
this file through its SQLite connection; Edge does not open it.

Apply schema and seed changes explicitly from the application root:

```sh
"$PHP" artisan migrate --database=sqlite --force
"$PHP" artisan db:seed --database=sqlite --force
```

Requests do not run migrations. Keep SQLite WAL and shared-memory files untracked.
