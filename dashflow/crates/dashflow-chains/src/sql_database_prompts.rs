//! SQL Database Chain Prompts
//!
//! Contains prompts for different SQL dialects optimized for query generation.

use dashflow::core::prompts::ChatPromptTemplate;

/// Suffix added to all SQL prompts
pub const PROMPT_SUFFIX: &str = r"Only use the following tables:
{table_info}

Question: {input}";

/// Default template for generic SQL databases
const DEFAULT_TEMPLATE: &str = r"Given an input question, first create a syntactically correct {dialect} query to run, then look at the results of the query and return the answer. Unless the user specifies in his question a specific number of examples he wishes to obtain, always limit your query to at most {top_k} results. You can order the results by a relevant column to return the most interesting examples in the database.

Never query for all the columns from a specific table, only ask for a few relevant columns given the question.

Pay attention to use only the column names that you can see in the schema description. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

";

/// PostgreSQL-specific template
const POSTGRES_TEMPLATE: &str = r#"You are a PostgreSQL expert. Given an input question, first create a syntactically correct PostgreSQL query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per PostgreSQL. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use CURRENT_DATE function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// MySQL-specific template
const MYSQL_TEMPLATE: &str = r#"You are a MySQL expert. Given an input question, first create a syntactically correct MySQL query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per MySQL. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in backticks (`) to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use CURDATE() function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// SQLite-specific template
const SQLITE_TEMPLATE: &str = r#"You are a SQLite expert. Given an input question, first create a syntactically correct SQLite query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per SQLite. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use date('now') function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// MSSQL-specific template
const MSSQL_TEMPLATE: &str = r#"You are an MS SQL expert. Given an input question, first create a syntactically correct MS SQL query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the TOP clause as per MS SQL. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in square brackets ([]) to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use CAST(GETDATE() as date) function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// Oracle-specific template
const ORACLE_TEMPLATE: &str = r#"You are an Oracle SQL expert. Given an input question, first create a syntactically correct Oracle SQL query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the FETCH FIRST n ROWS ONLY clause as per Oracle SQL. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use TRUNC(SYSDATE) function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// MariaDB-specific template
const MARIADB_TEMPLATE: &str = r#"You are a MariaDB expert. Given an input question, first create a syntactically correct MariaDB query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per MariaDB. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in backticks (`) to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use CURDATE() function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// DuckDB-specific template
const DUCKDB_TEMPLATE: &str = r#"You are a DuckDB expert. Given an input question, first create a syntactically correct DuckDB query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per DuckDB. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use today() function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// ClickHouse-specific template
const CLICKHOUSE_TEMPLATE: &str = r#"You are a ClickHouse expert. Given an input question, first create a syntactically correct ClickHouse query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per ClickHouse. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use today() function to get the current date, if the question involves "today".

Use the following format:

Question: "Question here"
SQLQuery: "SQL Query to run"
SQLResult: "Result of the SQLQuery"
Answer: "Final answer here"

"#;

/// CrateDB-specific template
const CRATEDB_TEMPLATE: &str = r#"You are a CrateDB expert. Given an input question, first create a syntactically correct CrateDB query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per CrateDB. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use CURRENT_DATE function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// GoogleSQL-specific template
const GOOGLESQL_TEMPLATE: &str = r#"You are a GoogleSQL expert. Given an input question, first create a syntactically correct GoogleSQL query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per GoogleSQL. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in backticks (`) to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use CURRENT_DATE() function to get the current date, if the question involves "today".

Use the following format:

Question: Question here
SQLQuery: SQL Query to run
SQLResult: Result of the SQLQuery
Answer: Final answer here

"#;

/// PrestoDB-specific template
const PRESTODB_TEMPLATE: &str = r#"You are a PrestoDB expert. Given an input question, first create a syntactically correct PrestoDB query to run, then look at the results of the query and return the answer to the input question.
Unless the user specifies in the question a specific number of examples to obtain, query for at most {top_k} results using the LIMIT clause as per PrestoDB. You can order the results to return the most informative data in the database.
Never query for all columns from a table. You must query only the columns that are needed to answer the question. Wrap each column name in double quotes (") to denote them as delimited identifiers.
Pay attention to use only the column names you can see in the tables below. Be careful to not query for columns that do not exist. Also, pay attention to which column is in which table.
Pay attention to use current_date function to get the current date, if the question involves "today".

Use the following format:

Question: "Question here"
SQLQuery: "SQL Query to run"
SQLResult: "Result of the SQLQuery"
Answer: "Final answer here"

"#;

/// Get the appropriate prompt template for a given SQL dialect
#[must_use]
pub fn get_prompt_for_dialect(dialect: &str) -> String {
    let template = match dialect.to_lowercase().as_str() {
        "postgresql" | "postgres" => POSTGRES_TEMPLATE,
        "mysql" => MYSQL_TEMPLATE,
        "sqlite" => SQLITE_TEMPLATE,
        "mssql" | "sqlserver" => MSSQL_TEMPLATE,
        "oracle" => ORACLE_TEMPLATE,
        "mariadb" => MARIADB_TEMPLATE,
        "duckdb" => DUCKDB_TEMPLATE,
        "clickhouse" => CLICKHOUSE_TEMPLATE,
        "crate" | "cratedb" => CRATEDB_TEMPLATE,
        "googlesql" => GOOGLESQL_TEMPLATE,
        "prestodb" => PRESTODB_TEMPLATE,
        _ => DEFAULT_TEMPLATE,
    };

    format!("{template}{PROMPT_SUFFIX}")
}

/// Create a `ChatPromptTemplate` for SQL query generation
#[must_use]
pub fn create_sql_prompt(dialect: &str) -> ChatPromptTemplate {
    let template = get_prompt_for_dialect(dialect);
    // Create a simple human message with the template
    #[allow(clippy::expect_used)]
    let prompt = ChatPromptTemplate::from_messages(vec![("human", &template)])
        .expect("Failed to create SQL prompt template");
    prompt
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_prompt_for_dialect() {
        // Test that we get dialect-specific prompts
        let pg_prompt = get_prompt_for_dialect("postgresql");
        assert!(pg_prompt.contains("PostgreSQL expert"));

        let mysql_prompt = get_prompt_for_dialect("mysql");
        assert!(mysql_prompt.contains("MySQL expert"));

        let sqlite_prompt = get_prompt_for_dialect("sqlite");
        assert!(sqlite_prompt.contains("SQLite expert"));

        // Test default
        let unknown_prompt = get_prompt_for_dialect("unknown");
        assert!(unknown_prompt.contains("{dialect}"));
    }

    #[test]
    fn test_all_prompts_have_required_variables() {
        let dialects = vec![
            "postgresql",
            "mysql",
            "sqlite",
            "mssql",
            "oracle",
            "mariadb",
            "duckdb",
            "clickhouse",
            "crate",
            "googlesql",
            "prestodb",
            "unknown",
        ];

        for dialect in dialects {
            let prompt = get_prompt_for_dialect(dialect);
            assert!(
                prompt.contains("{input}"),
                "Missing {{input}} in {}",
                dialect
            );
            assert!(
                prompt.contains("{table_info}"),
                "Missing {{table_info}} in {}",
                dialect
            );
            assert!(
                prompt.contains("{top_k}"),
                "Missing {{top_k}} in {}",
                dialect
            );
        }
    }
}
