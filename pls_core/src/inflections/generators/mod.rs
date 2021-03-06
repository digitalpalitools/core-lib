use crate::inflections::{InflectionClass, Pali1Metadata};

mod conjugation;
mod declension;
mod declension_pron_1st;
mod declension_pron_2nd;
mod declension_pron_dual;

pub fn create_html_body(
    pm: &Pali1Metadata,
    transliterate: fn(&str) -> Result<String, String>,
    exec_sql: impl Fn(String) -> Result<Vec<Vec<Vec<String>>>, String>,
) -> Result<String, String> {
    let table_name = pm.pattern.replace(" ", "_");
    match pm.inflection_class {
        InflectionClass::Conjugation => {
            conjugation::create_html_body(&table_name, &pm.stem, transliterate, exec_sql)
        }
        InflectionClass::Declension => {
            declension::create_html_body(&table_name, &pm.stem, transliterate, exec_sql)
        }
        InflectionClass::DeclensionPron1st => {
            declension_pron_1st::create_html_body(&table_name, &pm.stem, transliterate, exec_sql)
        }
        InflectionClass::DeclensionPron2nd => {
            declension_pron_2nd::create_html_body(&table_name, &pm.stem, transliterate, exec_sql)
        }
        InflectionClass::DeclensionPronDual => {
            declension_pron_dual::create_html_body(&table_name, &pm.stem, transliterate, exec_sql)
        }
    }
}
