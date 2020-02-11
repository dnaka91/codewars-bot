use anyhow::Result;
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "commands.pest"]
struct CommandParser;

#[derive(Debug)]
pub enum Command {
    AddUser(String),
    RemoveUser(String),
    Stats,
}

pub fn parse(cmd: &str) -> Result<Command> {
    let command = CommandParser::parse(Rule::command, cmd)?.next().unwrap();
    let command = command.into_inner().next().unwrap();

    Ok(match command.as_rule() {
        Rule::add => Command::AddUser(command.into_inner().next().unwrap().as_str().to_owned()),
        Rule::remove => {
            Command::RemoveUser(command.into_inner().next().unwrap().as_str().to_owned())
        }
        Rule::stats => Command::Stats,
        _ => unreachable!(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let command = CommandParser::parse(Rule::command, "add dnaka91")
            .unwrap()
            .next()
            .unwrap();
        let add = command.into_inner().next().unwrap();
        let username = add.into_inner().next().unwrap();

        assert_eq!("dnaka91", username.as_str());
    }
}
