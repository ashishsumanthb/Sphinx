# Sphinx presentation demo

## Start the demo

From the repository root:

```bash
cp -n .env.example .env
docker compose up -d --wait db
cargo run --locked --bin seed_demo
cargo run --locked --bin sphinx
```

In a second terminal, serve the frontend:

```bash
nix-shell -p python3 --run 'python3 -m http.server 3000 --directory frontend/dist'
```

Open <http://127.0.0.1:3000>.

## Accounts

Every demo account uses the password `SphinxDemo123!`.

| Role | Name | Email |
| --- | --- | --- |
| Admin | Srikanth | `srikanth@sphinx.local` |
| Member | Arshad | `arshad@sphinx.local` |
| Member | Ashish | `ashish@sphinx.local` |
| Member | Sushma | `sushma@sphinx.local` |
| Member | Gangadhar | `gangadhar@sphinx.local` |
| Member | Abhishek | `abhishek@sphinx.local` |
| Member | Aditya | `aditya@sphinx.local` |
| Member | Sanchey | `sanchey@sphinx.local` |
| Member | Sowjanya | `sowjanya@sphinx.local` |
| Member | Sheryas | `sheryas@sphinx.local` |
| Member | Sathvik | `sathvik@sphinx.local` |

## Presentation path

1. Log in as Srikanth.
2. Select **Sphinx Presentation Team**.
3. Enter the quiz details and choose one or two attempts.
4. Show the ten checked participants. Srikanth is excluded because administrators are not quiz participants.
5. Use the three prepared questions or add a custom question. Custom questions are checked by the Rust domain validator before they enter the prepared set.
6. Select **Create quiz, assign members & publish**.
7. Copy the published quiz ID if needed, then log out.
8. Log in as one of the ten members.
9. Select **Sphinx Presentation Team**, choose the available quiz and submit answers.
10. Show the saved score, attempt history and revision-specific rank.

Create the presentation quiz during the demo so its availability window starts at that moment.
