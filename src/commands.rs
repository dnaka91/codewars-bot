use anyhow::{anyhow, bail, Result};
use chrono::{NaiveTime, Weekday};
use pest::Parser;
use pest_derive::Parser;

#[derive(Parser)]
#[grammar = "commands.pest"]
struct CommandParser;

#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub enum Command {
    AddUser(String),
    RemoveUser(String),
    Stats,
    Help,
    Schedule(Weekday, NaiveTime),
}

pub fn parse(cmd: &str) -> Result<Command> {
    let command = CommandParser::parse(Rule::command, cmd)?
        .next()
        .ok_or_else(|| anyhow!("command missing"))?;
    let command = command
        .into_inner()
        .next()
        .ok_or_else(|| anyhow!("command missing"))?;

    Ok(match command.as_rule() {
        Rule::add => Command::AddUser(
            command
                .into_inner()
                .next()
                .ok_or_else(|| anyhow!("username missing"))?
                .as_str()
                .to_owned(),
        ),
        Rule::remove => Command::RemoveUser(
            command
                .into_inner()
                .next()
                .ok_or_else(|| anyhow!("username missing"))?
                .as_str()
                .to_owned(),
        ),
        Rule::stats => Command::Stats,
        Rule::help => Command::Help,
        Rule::schedule => {
            let mut args = command.into_inner();
            Command::Schedule(
                args.next()
                    .ok_or_else(|| anyhow!("weekday missing"))?
                    .as_str()
                    .parse()
                    .map_err(|_| anyhow!("invalid weekday"))?,
                args.next().map_or_else(
                    || Ok(NaiveTime::from_hms(10, 0, 0)),
                    |t| NaiveTime::parse_from_str(t.as_str(), "%R"),
                )?,
            )
        }
        _ => bail!("unknown command"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_add() {
        assert_eq!(
            Some(Command::AddUser("him".to_owned())),
            parse("add him").ok()
        );
    }

    #[test]
    fn parse_remove() {
        assert_eq!(
            Some(Command::RemoveUser("him".to_owned())),
            parse("remove him").ok()
        );
        assert_eq!(
            Some(Command::RemoveUser("him".to_owned())),
            parse("rm him").ok()
        );
    }

    #[test]
    fn parse_stats() {
        assert_eq!(Some(Command::Stats), parse("stats").ok());
    }

    #[test]
    fn parse_help() {
        assert_eq!(Some(Command::Help), parse("help").ok());
    }

    #[test]
    fn parse_schedule() {
        assert_eq!(
            Some(Command::Schedule(
                Weekday::Wed,
                NaiveTime::from_hms(13, 5, 0)
            )),
            parse("schedule on Wednesday at 13:05").ok()
        );
        assert_eq!(
            Some(Command::Schedule(
                Weekday::Wed,
                NaiveTime::from_hms(10, 0, 0)
            )),
            parse("schedule on Wednesday").ok()
        );
    }
}
