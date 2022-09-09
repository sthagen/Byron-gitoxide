fn main() -> Result<(), git_prompt::Error> {
    let user = git_prompt::openly("Username: ")?;
    eprintln!("{user:?}");
    let pass = git_prompt::securely("Password: ")?;
    eprintln!("{pass:?}");
    Ok(())
}
