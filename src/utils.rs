pub fn run<S: AsRef<str>>(command: S) -> anyhow::Result<String> {
    use execute::Execute;
    use std::process::Stdio;

    log::info!("Executing '{}'", command.as_ref());
    let mut command = execute::command(command.as_ref());
    command.stdout(Stdio::piped());

    let output = command.execute_output()?;
    anyhow::ensure!(
        output.status.success(),
        "command failed with status {:?}",
        output.status.code()
    );

    Ok(String::from_utf8(output.stdout)?)
}
