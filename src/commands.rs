//! Command parser to turn text messages into comamnds for the service.

use chrono::{NaiveDate, NaiveTime, Weekday};
use pest::Parser;
use pest_derive::Parser;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Command missing")]
    CommandMissing,
    #[error("Username missing")]
    UsernameMissing,
    #[error("Weekday missing")]
    WeekdayMissing,
    #[error("Boolean missing")]
    BooleanMissing,
    #[error("Failed parsing date or time")]
    InvalidDateTime(#[from] chrono::ParseError),
    #[error("Invalid weekday")]
    InvalidWeekday(chrono::ParseWeekdayError),
    #[error("Invalid boolean")]
    InvalidBoolean,
    #[error("Unknown command")]
    UnknownCommand,
    #[error("Invalid command input")]
    InvalidInput(#[from] pest::error::Error<Rule>),
}

/// The actual parser that uses PEST grammar to parse text messages.
#[derive(Parser)]
#[grammar = "commands.pest"]
struct CommandParser;

/// All possible supported commands that are understood by the service.
#[derive(Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub enum Command {
    /// Add a user to the tracking list.
    AddUser(String),
    /// Stop tracking a user.
    RemoveUser(String),
    /// Get and report Codewars statistics with optional start date.
    Stats(Option<NaiveDate>),
    /// Show a help message.
    Help,
    /// Update the schedule for weekly reports.
    Schedule(Weekday, NaiveTime),
    /// Turn automatic notifications of new challenges on or off.
    Notify(bool),
}

/// Parse a text message into one of the possible commands that the service understands.
pub fn parse(cmd: &str) -> Result<Command> {
    let command = CommandParser::parse(Rule::command, cmd)?
        .next()
        .ok_or(Error::CommandMissing)?;
    let command = command.into_inner().next().ok_or(Error::CommandMissing)?;

    Ok(match command.as_rule() {
        Rule::add => Command::AddUser(
            command
                .into_inner()
                .next()
                .ok_or(Error::UsernameMissing)?
                .as_str()
                .to_owned(),
        ),
        Rule::remove => Command::RemoveUser(
            command
                .into_inner()
                .next()
                .ok_or(Error::UsernameMissing)?
                .as_str()
                .to_owned(),
        ),
        Rule::stats => {
            let mut args = command.into_inner();
            Command::Stats(args.next().map_or_else(
                || Ok(None),
                |d| NaiveDate::parse_from_str(d.as_str(), "%Y/%m/%d").map(Some),
            )?)
        }
        Rule::help => Command::Help,
        Rule::schedule => {
            let mut args = command.into_inner();
            Command::Schedule(
                args.next()
                    .ok_or(Error::WeekdayMissing)?
                    .as_str()
                    .parse()
                    .map_err(Error::InvalidWeekday)?,
                args.next().map_or_else(
                    || Ok(NaiveTime::from_hms(10, 0, 0)),
                    |t| NaiveTime::parse_from_str(t.as_str(), "%R"),
                )?,
            )
        }
        Rule::notify => {
            let boolean = command
                .into_inner()
                .next()
                .ok_or(Error::BooleanMissing)?
                .as_str();
            let on_off = match boolean {
                "on" => true,
                "off" => false,
                _ => return Err(Error::InvalidBoolean),
            };
            Command::Notify(on_off)
        }
        _ => return Err(Error::UnknownCommand),
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
        assert_eq!(Some(Command::Stats(None)), parse("stats").ok());
        assert_eq!(
            Some(Command::Stats(Some(NaiveDate::from_ymd(2020, 2, 5)))),
            parse("stats since 2020/02/05").ok()
        );
        assert_eq!(
            Some(Command::Stats(Some(NaiveDate::from_ymd(2020, 1, 3)))),
            parse("stats since 2020/1/3").ok()
        );
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
                Weekday::Tue,
                NaiveTime::from_hms(10, 0, 0)
            )),
            parse("schedule on Tue").ok()
        );
    }

    #[test]
    fn parse_notify() {
        assert_eq!(Some(Command::Notify(true)), parse("notify on").ok())
    }
}
