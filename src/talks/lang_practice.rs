/**************************************************************************
  Copyright 2026 Francesco Versaci (https://github.com/fversaci/)

  This program is free software: you can redistribute it and/or modify
  it under the terms of the GNU Affero General Public License as published by
  the Free Software Foundation, either version 3 of the License, or
  (at your option) any later version.

  This program is distributed in the hope that it will be useful,
  but WITHOUT ANY WARRANTY; without even the implied warranty of
  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
  GNU Affero General Public License for more details.

  You should have received a copy of the GNU Affero General Public License
  along with this program.  If not, see <https://www.gnu.org/licenses/>.
**************************************************************************/
use clap::ValueEnum;
use strum_macros::{Display, EnumIter, EnumString};

#[derive(Default, Display, Debug, Clone, EnumIter, EnumString, ValueEnum)]
pub enum Lang {
    #[default]
    English,
    German,
    French,
    Spanish,
    Italian,
}

impl Lang {
    pub fn iso_639_1(&self) -> &'static str {
        match self {
            Lang::English => "en",
            Lang::German => "de",
            Lang::French => "fr",
            Lang::Spanish => "es",
            Lang::Italian => "it",
        }
    }
}

#[derive(Default, Display, Debug, Clone, EnumIter, EnumString, ValueEnum)]
pub enum LangLevel {
    #[default]
    Beginner,
    Intermediate,
    Advanced,
}
