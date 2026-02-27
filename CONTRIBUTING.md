# Notes for contributors

First, thank you for your interest in contributing to `pixivdwn`! We greatly appreciate any help, whether it's fixing a typo in the documentation, or implementing a new feature.

This document contains some notes for contributors, including how to set up their development environments, and the overview for database layout.

## Development environment

`pixivdwn` uses `sqlx` for database access. Its `query!` macro does type-checking at compile time, but requires a live database connection to do so. In order to have a local database for development:

```bash
# Install the sqlx CLI tool
cargo install sqlx-cli

# Set DATABASE_URL to ./db.sqlite
echo "DATABASE_URL=sqlite://./db.sqlite" >> .env

# Create a local SQLite database, and run all migrations
sqlx database setup
```

It's also a good idea to run `sqlx database reset` whenever you pull or switch branches. If you want to keep the existing data and is sure that the database schema is only updated, you can use `sqlx migrate run`.

One more thing: `sqlx` can uses "offline" information pre-generated to type-check `query!` macros without a live database connection. This is what CI uses to compile the code without setting up a live database. So if you added new migrations, please run `cargo sqlx prepare` and commit the changes in `/.sqlx` as well.