mod generators;

use crate::alphabet::string_compare;
use regex::{Error, Regex};
use serde::Serialize;
use std::collections::HashMap;
use tera::{Context, Tera, Value};

lazy_static! {
    static ref TEMPLATES: Tera = {
        let mut tera = Tera::default();
        tera.add_raw_templates(vec![("output", include_str!("templates/output.html"))])
            .expect("Unexpected failure adding template");
        tera.autoescape_on(vec!["html"]);
        tera
    };
    static ref INDECLINABLE_CRACKER: Result<Regex, Error> = Regex::new(r" \d+$");
}

#[derive(Debug)]
pub enum InflectionClass {
    Indeclinable,
    Conjugation,
    Declension,
    DeclensionPron1st,
    DeclensionPron2nd,
    DeclensionPronDual,
}

pub struct Pali1Metadata {
    pub pali1: String,
    pub stem: String,
    pub pattern: String,
    pub pos: String,
    pub meaning: String,
    pub inflection_class: InflectionClass,
    pub like: String,
}

pub trait PlsInflectionsHost<'a> {
    fn get_locale(&self) -> &'a str;
    fn get_version(&self) -> &'a str;
    fn get_url(&self) -> &'a str;
    fn transliterate(&self, s: &str) -> Result<String, String>;
    fn exec_sql_query_core(&self, sql: &str) -> Result<String, String>;
    fn exec_sql_query(&self, sql: &str) -> Result<Vec<Vec<Vec<String>>>, String> {
        let result_str = self.exec_sql_query_core(sql)?;
        let result: Vec<Vec<Vec<String>>> =
            serde_json::from_str(&result_str).map_err(|e| e.to_string())?;
        Ok(result)
    }
    fn log_warning(&self, msg: &str);
}

pub fn generate_inflection_table(
    pali1: &str,
    host: &dyn PlsInflectionsHost,
) -> Result<String, String> {
    let pm = get_pali1_metadata(pali1, host)?;
    let body = generators::create_html_body(&pm, host)?;

    generate_output(&pm, pali1, &body, host)
}

pub fn generate_all_inflections(
    pali1: &str,
    host: &dyn PlsInflectionsHost,
) -> Result<Vec<String>, String> {
    let pm = get_pali1_metadata(pali1, host)?;
    let table_name = get_table_name_from_pattern(&pm.pattern);

    let inflected_words: Vec<String> = match pm.stem.as_ref() {
        "-" => get_all_inflections_for_indeclinables(pali1)?,
        "*" => get_all_inflections_for_irregulars(&table_name, host)?,
        _ => get_all_inflections_for_regulars(&pm.stem, &table_name, host)?,
    };

    Ok(inflected_words)
}

fn get_table_name_from_pattern(pattern: &str) -> String {
    pattern.replace(" ", "_")
}

fn inflection_class_from_str(ic: &str) -> InflectionClass {
    match ic {
        "verb" => InflectionClass::Conjugation,
        "pron1st" => InflectionClass::DeclensionPron1st,
        "pron2nd" => InflectionClass::DeclensionPron2nd,
        "prondual" => InflectionClass::DeclensionPronDual,
        _ => InflectionClass::Declension,
    }
}

fn get_stem_for_indeclinable(pali1: &str) -> Result<String, String> {
    let regex = INDECLINABLE_CRACKER.as_ref().map_err(|e| e.to_string())?;
    Ok(regex.replace(pali1, "").to_string())
}

fn get_pali1_metadata(pali1: &str, host: &dyn PlsInflectionsHost) -> Result<Pali1Metadata, String> {
    let sql = format!(
        r#"select stem, pattern, pos, definition from '_stems' where pāli1 = "{}""#,
        pali1,
    );
    let results = host.exec_sql_query(&sql)?;
    if results.len() != 1 || results[0].len() != 1 || results[0][0].len() != 4 {
        return Err(format!("Word '{}' not found in db.", pali1));
    }

    let stem = &results[0][0][0];
    let pattern = &results[0][0][1];
    let mut pm = Pali1Metadata {
        pali1: pali1.to_string(),
        stem: if !stem.eq("*") {
            stem.to_owned()
        } else {
            "".to_string()
        },
        pattern: pattern.to_owned(),
        pos: results[0][0][2].to_owned(),
        meaning: results[0][0][3].to_owned(),
        inflection_class: InflectionClass::Declension,
        like: "".to_string(),
    };

    if !pattern.trim().is_empty() {
        let sql = format!(
            r#"select inflection_class, like from '_index' where name = "{}""#,
            pattern
        );
        let results = host.exec_sql_query(&sql)?;
        let inflection_class = &results[0][0][0];
        let like = &results[0][0][1];

        pm.inflection_class = inflection_class_from_str(inflection_class);
        pm.like = if !like.is_empty() {
            format!("like {}", host.transliterate(like)?)
        } else {
            "irreg".to_string()
        };
    } else if stem.eq("-") {
        pm.inflection_class = InflectionClass::Indeclinable;
        pm.pali1 = get_stem_for_indeclinable(&pm.pali1)?;
        pm.like = "indeclinable".to_string();
    }

    Ok(pm)
}

#[derive(Serialize)]
struct ViewModel<'a> {
    pub pali1: &'a str,
    pub pattern: &'a str,
    pub like: &'a str,
    pub pos: &'a str,
    pub meaning: &'a str,
    pub body: &'a str,
    pub feedback_form_url: &'a str,
    pub host_url: &'a str,
    pub host_version: &'a str,
}

fn generate_output(
    pm: &Pali1Metadata,
    pali1: &str,
    body: &str,
    host: &dyn PlsInflectionsHost,
) -> Result<String, String> {
    let feedback_form_url = match pm.inflection_class {
        InflectionClass::Conjugation => {
            "https://docs.google.com/forms/d/e/1FAIpQLSeJpx7TsISkYEXzxvbBtOH25T-ZO1Z5NFdujO5SD9qcAH_i1A/viewform"
        }
        _ => { // All declensions.
            "https://docs.google.com/forms/d/e/1FAIpQLSeoxZiqvIWadaLeuXF4f44NCqEn49-B8KNbSvNer5jxgRYdtQ/viewform"
        }
    };

    let vm = ViewModel {
        pali1: &host.transliterate(pali1)?,
        pattern: &pm.pattern,
        like: &pm.like,
        pos: &pm.pos,
        meaning: &pm.meaning,
        body: &body,
        feedback_form_url: &feedback_form_url,
        host_url: host.get_url(),
        host_version: host.get_version(),
    };

    let context = Context::from_serialize(&vm).map_err(|e| e.to_string())?;
    TEMPLATES
        .render("output", &context)
        .map_err(|e| e.to_string())
}

fn get_inflection_suffixes_for_pattern(
    pattern: &str,
    host: &dyn PlsInflectionsHost,
) -> Result<Vec<Vec<Vec<String>>>, String> {
    host.exec_sql_query(&format!("Select * from {}", pattern))
}

fn get_all_inflections_for_indeclinables(pali1: &str) -> Result<Vec<String>, String> {
    Ok(vec![get_stem_for_indeclinable(pali1)?])
}

fn get_all_inflections_for_irregulars(
    pattern: &str,
    host: &dyn PlsInflectionsHost,
) -> Result<Vec<String>, String> {
    let suffixes: Vec<Vec<String>> = get_inflection_suffixes_for_pattern(pattern, host)?
        .pop()
        .ok_or_else(|| format!("No pattern found for {}", pattern))?;
    let mut inflections: Vec<String> = Vec::new();
    for mut suffix_row in suffixes {
        for suffix in suffix_row
            .pop()
            .ok_or_else(|| format!("No pattern found for {}", pattern))?
            .split(',')
        {
            inflections.push(suffix.to_string())
        }
    }
    Ok(inflections)
}

fn get_all_inflections_for_regulars(
    stem: &str,
    pattern: &str,
    host: &dyn PlsInflectionsHost,
) -> Result<Vec<String>, String> {
    let mut inflections: Vec<String> = Vec::new();
    let suffixes: Vec<Vec<String>> = get_inflection_suffixes_for_pattern(pattern, host)?
        .pop()
        .ok_or_else(|| format!("No pattern found for {}", pattern))?;
    for mut suffix_row in suffixes {
        for suffix in suffix_row
            .pop()
            .ok_or_else(|| format!("No pattern found for {}", pattern))?
            .split(',')
        {
            inflections.push(format!("{}{}", stem, suffix))
        }
    }
    Ok(inflections)
}

pub fn localise_abbrev(value: &Value, arg: &HashMap<String, Value>) -> tera::Result<Value> {
    let localised_abbrev = &arg["hmap"][value
        .as_str()
        .ok_or_else(|| "Error while converting value to str.".to_string())?];
    if localised_abbrev.is_null() {
        let error_string = format!("Error: abbreviation not found for {}", value);
        println!("{}", error_string);
        return Err(tera::Error::msg(error_string));
    }
    Ok(serde_json::value::to_value(localised_abbrev)?)
}

fn join_and_transliterate_if_not_empty(
    stem: &str,
    suffix: &str,
    host: &dyn PlsInflectionsHost,
) -> String {
    if suffix.is_empty() {
        "".to_string()
    } else {
        host.transliterate(&format!("{}{}", stem, suffix))
            .unwrap_or_else(|e| e)
    }
}

fn get_inflections(stem: &str, sql: &str, host: &dyn PlsInflectionsHost) -> Vec<String> {
    let res = match host.exec_sql_query(&sql) {
        Ok(x) => {
            if x.len() == 1 && x[0].len() == 1 && x[0][0].len() == 1 {
                x[0][0][0].to_string()
            } else {
                "".to_string()
            }
        }
        Err(e) => e,
    };

    let mut inflections: Vec<String> = res
        .split(',')
        .map(|s| join_and_transliterate_if_not_empty(stem, s, host))
        .collect();
    inflections.sort_by(|a, b| Ord::cmp(&string_compare(a, b), &0));
    inflections
}

fn query_has_no_results(query: &str, host: &dyn PlsInflectionsHost) -> Result<bool, String> {
    let count = &host.exec_sql_query(&query)?[0][0][0];
    Ok(count.eq("0"))
}

pub fn get_abbreviations_for_locale(
    host: &dyn PlsInflectionsHost,
) -> Result<HashMap<String, String>, String> {
    let locale = host.get_locale();
    let sql = if locale == "xx" {
        "select name, description, '^' || name || '$' from _abbreviations".to_string()
    } else if locale == "en" {
        "select name, description, name from _abbreviations".to_string()
    } else {
        format!(
            r#"select name, description, {} from _abbreviations"#,
            locale
        )
    };
    let res = host.exec_sql_query(&sql)?;
    let mut abbrev_map = HashMap::new();
    for i in res[0].iter() {
        abbrev_map.insert(i[0].clone(), i[2].clone());
        abbrev_map.insert(i[1].clone(), i[2].clone());
    }
    Ok(abbrev_map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{Connection, Row, NO_PARAMS};
    use test_case::test_case;

    struct Host<'a> {
        locale: &'a str,
        version: &'a str,
        url: &'a str,
        psuedo_transliterate: bool,
    }

    impl<'a> PlsInflectionsHost<'a> for Host<'a> {
        fn get_locale(&self) -> &'a str {
            self.locale
        }

        fn get_version(&self) -> &'a str {
            self.version
        }

        fn get_url(&self) -> &'a str {
            self.url
        }

        fn transliterate(&self, s: &str) -> Result<String, String> {
            let ret = if self.psuedo_transliterate {
                format!("^{}$", s)
            } else {
                s.to_string()
            };

            Ok(ret)
        }

        fn exec_sql_query_core(&self, sql: &str) -> Result<String, String> {
            let table = exec_sql_core(&sql).map_err(|x| x.to_string())?;
            serde_json::to_string(&table).map_err(|x| x.to_string())
        }

        fn log_warning(&self, msg: &str) {
            println!("WARNING: {}", msg)
        }
    }

    fn get_row_cells(row: &Row) -> Vec<String> {
        let cells: Vec<String> = row
            .column_names()
            .iter()
            .map(|&cn| {
                let cell: String = match row.get(cn) {
                    Ok(c) => c,
                    Err(e) => e.to_string(),
                };
                cell
            })
            .collect();

        cells
    }

    fn exec_sql_core(sql: &str) -> rusqlite::Result<Vec<Vec<Vec<String>>>, rusqlite::Error> {
        let conn = Connection::open("../inflections.db")?;
        let mut result: Vec<Vec<Vec<String>>> = Vec::new();
        for s in sql.split(';').filter(|s| !s.trim().is_empty()) {
            let mut stmt = conn.prepare(&s)?;
            let mut rows = stmt.query(NO_PARAMS)?;

            let mut table: Vec<Vec<String>> = Vec::new();
            while let Some(row) = rows.next()? {
                table.push(get_row_cells(row));
            }
            result.push(table)
        }

        Ok(result)
    }

    #[test_case("ābādheti","xx"; "conjugation - 1 - xx")]
    #[test_case("vassūpanāyikā","xx"; "declension - 1 - xx ")]
    #[test_case("kamma 1","xx"; "declension - 2 - irreg - xx")]
    #[test_case("kāmaṃ 3","xx"; "declension - 3 - ind - xx")]
    #[test_case("ubha","xx"; "declension - 4 - pron_dual - xx")]
    #[test_case("maṃ","xx"; "declension - 4 - pron_1st - xx")]
    #[test_case("taṃ 3","xx"; "declension - 4 - pron_2nd - xx")]
    #[test_case("pañca","xx"; "declension - 5 - only x gender - xx")]
    #[test_case("ābādheti","en"; "conjugation - 1 - en")]
    #[test_case("xyz","xxx"; "word that does not exist")]
    fn inflection_tests(pali1: &str, locale: &str) {
        let html = generate_inflection_table(
            pali1,
            &Host {
                locale,
                url: "test case",
                version: "v0.1",
                psuedo_transliterate: true,
            },
        )
        .unwrap_or_else(|e| e);
        insta::assert_snapshot!(html);
    }

    #[test_case("a 1"; "indeclinable")]
    #[test_case("ababa 1"; "regular")]
    #[test_case("ahesuṃ"; "irregular")]
    fn inflected_word_tests(pali1: &str) {
        let host = Host {
            locale: "en",
            url: "test case",
            version: "v0.1",
            psuedo_transliterate: true,
        };

        let output: Vec<String> =
            generate_all_inflections(pali1, &host).unwrap_or_else(|_e| Vec::new());

        insta::assert_yaml_snapshot!(output);
    }

    #[test_case("xx", "missingAbbreviation")]
    #[test_case("xx", "pl")]
    #[test_case("en", "pl")]
    fn localise_abbrev_filter_test(locale: &str, word: &str) {
        let mut tera = Tera::default();
        tera.register_filter("localise_abbrev", localise_abbrev);
        tera.add_raw_templates(vec![(
            "test_file",
            include_str!("./generators/templates/test_file.html"),
        )])
        .expect("Unexpected failure adding template");
        tera.autoescape_on(vec!["html"]);

        let host = Host {
            locale,
            url: "test case",
            version: "v0.1",
            psuedo_transliterate: true,
        };

        let abbrev_map = get_abbreviations_for_locale(&host);
        let mut context = Context::new();
        context.insert("abbrev_map", &abbrev_map.ok());
        context.insert("word", &word);

        let html = tera
            .render("test_file", &context)
            .map_err(|e| e.to_string())
            .unwrap_or_else(|e| e);
        insta::assert_snapshot!(html);
    }
}
