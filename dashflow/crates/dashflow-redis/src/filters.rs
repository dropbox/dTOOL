//! Redis filter expressions for metadata filtering.
//!
//! This module provides a type-safe way to build Redis filter expressions
//! for metadata-based filtering during vector search.
//!
//! # Examples
//!
//! ```rust,ignore
//! use dashflow_redis::filters::*;
//!
//! // Tag filter
//! let brand = TagFilter::new("brand").eq("nike");
//! // Generates: @brand:{nike}
//!
//! // Numeric filter
//! let price = NumFilter::new("price").lt(100.0);
//! // Generates: @price:[-inf (100]
//!
//! // Text filter
//! let job = TextFilter::new("job").eq("engineer");
//! // Generates: @job:("engineer")
//!
//! // Combined filter (AND)
//! let filter = brand.and(price);
//! // Generates: (@brand:{nike} @price:[-inf (100])
//!
//! // Combined filter (OR)
//! let filter = brand.or(price);
//! // Generates: (@brand:{nike} | @price:[-inf (100])
//! ```

use std::fmt;

/// Filter operator type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterOperator {
    /// Equality operator (==)
    Eq,
    /// Inequality operator (!=)
    Ne,
    /// Less than operator (<)
    Lt,
    /// Greater than operator (>)
    Gt,
    /// Less than or equal operator (<=)
    Le,
    /// Greater than or equal operator (>=)
    Ge,
    /// Text matching with wildcards (%)
    Like,
    /// Membership operator for tag lists (==)
    In,
}

/// Escape special characters in Redis query strings.
fn escape_redis_value(s: &str) -> String {
    // Redis special characters that need escaping: , . < > { } [ ] " ' : ; ! @ # $ % ^ & * ( ) - + = ~ `
    // For tags, we mainly need to escape: , | { }
    // For simplicity, we'll replace problematic characters
    s.replace(',', "\\,")
        .replace('|', "\\|")
        .replace('{', "\\{")
        .replace('}', "\\}")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

/// Tag filter for categorical data (exact match).
///
/// Tag filters support equality and inequality operators on string tags.
#[derive(Debug, Clone)]
pub struct TagFilter {
    field: String,
    values: Vec<String>,
    operator: FilterOperator,
}

impl TagFilter {
    /// Create a new tag filter for the given field.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            values: Vec::new(),
            operator: FilterOperator::Eq,
        }
    }

    /// Create an equality filter (tag equals value).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = TagFilter::new("brand").eq("nike");
    /// // Generates: @brand:{nike}
    /// ```
    pub fn eq(mut self, value: impl Into<String>) -> FilterExpression {
        self.values = vec![value.into()];
        self.operator = FilterOperator::Eq;
        FilterExpression::Tag(self)
    }

    /// Create an inequality filter (tag not equals value).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = TagFilter::new("brand").ne("nike");
    /// // Generates: (-@brand:{nike})
    /// ```
    pub fn ne(mut self, value: impl Into<String>) -> FilterExpression {
        self.values = vec![value.into()];
        self.operator = FilterOperator::Ne;
        FilterExpression::Tag(self)
    }

    /// Create an "in" filter (tag in list of values).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = TagFilter::new("brand").in_values(vec!["nike", "adidas"]);
    /// // Generates: @brand:{nike|adidas}
    /// ```
    pub fn in_values<I, S>(mut self, values: I) -> FilterExpression
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.values = values.into_iter().map(std::convert::Into::into).collect();
        self.operator = FilterOperator::In;
        FilterExpression::Tag(self)
    }

    /// Generate the Redis query string for this filter.
    fn to_query_string(&self) -> String {
        if self.values.is_empty() {
            return "*".to_string();
        }

        // Join values with |
        let formatted_values = self
            .values
            .iter()
            .map(|v| escape_redis_value(v))
            .collect::<Vec<_>>()
            .join("|");

        match self.operator {
            FilterOperator::Eq | FilterOperator::In => {
                format!("@{}:{{{}}}", self.field, formatted_values)
            }
            FilterOperator::Ne => {
                format!("(-@{}:{{{}}})", self.field, formatted_values)
            }
            _ => unreachable!("TagFilter only supports Eq, Ne, In operators"),
        }
    }
}

/// Numeric filter for range queries.
///
/// Numeric filters support all comparison operators (==, !=, <, >, <=, >=).
#[derive(Debug, Clone)]
pub struct NumFilter {
    field: String,
    value: f64,
    operator: FilterOperator,
}

impl NumFilter {
    /// Create a new numeric filter for the given field.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            value: 0.0,
            operator: FilterOperator::Eq,
        }
    }

    /// Create an equality filter (value == number).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = NumFilter::new("price").eq(99.99);
    /// // Generates: @price:[99.99 99.99]
    /// ```
    #[must_use]
    pub fn eq(mut self, value: f64) -> FilterExpression {
        self.value = value;
        self.operator = FilterOperator::Eq;
        FilterExpression::Num(self)
    }

    /// Create an inequality filter (value != number).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = NumFilter::new("price").ne(99.99);
    /// // Generates: (-@price:[99.99 99.99])
    /// ```
    #[must_use]
    pub fn ne(mut self, value: f64) -> FilterExpression {
        self.value = value;
        self.operator = FilterOperator::Ne;
        FilterExpression::Num(self)
    }

    /// Create a less-than filter (value < number).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = NumFilter::new("price").lt(100.0);
    /// // Generates: @price:[-inf (100]
    /// ```
    #[must_use]
    pub fn lt(mut self, value: f64) -> FilterExpression {
        self.value = value;
        self.operator = FilterOperator::Lt;
        FilterExpression::Num(self)
    }

    /// Create a greater-than filter (value > number).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = NumFilter::new("price").gt(100.0);
    /// // Generates: @price:[(100 +inf]
    /// ```
    #[must_use]
    pub fn gt(mut self, value: f64) -> FilterExpression {
        self.value = value;
        self.operator = FilterOperator::Gt;
        FilterExpression::Num(self)
    }

    /// Create a less-than-or-equal filter (value <= number).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = NumFilter::new("price").le(100.0);
    /// // Generates: @price:[-inf 100]
    /// ```
    #[must_use]
    pub fn le(mut self, value: f64) -> FilterExpression {
        self.value = value;
        self.operator = FilterOperator::Le;
        FilterExpression::Num(self)
    }

    /// Create a greater-than-or-equal filter (value >= number).
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = NumFilter::new("price").ge(100.0);
    /// // Generates: @price:[100 +inf]
    /// ```
    #[must_use]
    pub fn ge(mut self, value: f64) -> FilterExpression {
        self.value = value;
        self.operator = FilterOperator::Ge;
        FilterExpression::Num(self)
    }

    /// Generate the Redis query string for this filter.
    fn to_query_string(&self) -> String {
        match self.operator {
            FilterOperator::Eq => format!("@{}:[{} {}]", self.field, self.value, self.value),
            FilterOperator::Ne => {
                format!("(-@{}:[{} {}])", self.field, self.value, self.value)
            }
            FilterOperator::Lt => format!("@{}:[-inf ({}]", self.field, self.value),
            FilterOperator::Gt => format!("@{}:[({} +inf]", self.field, self.value),
            FilterOperator::Le => format!("@{}:[-inf {}]", self.field, self.value),
            FilterOperator::Ge => format!("@{}:[{} +inf]", self.field, self.value),
            _ => unreachable!("NumFilter only supports comparison operators"),
        }
    }
}

/// Text filter for full-text search.
///
/// Text filters support exact match, inequality, and LIKE (wildcard) operators.
#[derive(Debug, Clone)]
pub struct TextFilter {
    field: String,
    value: String,
    operator: FilterOperator,
}

impl TextFilter {
    /// Create a new text filter for the given field.
    pub fn new(field: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            value: String::new(),
            operator: FilterOperator::Eq,
        }
    }

    /// Create an exact match filter.
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = TextFilter::new("job").eq("engineer");
    /// // Generates: @job:("engineer")
    /// ```
    pub fn eq(mut self, value: impl Into<String>) -> FilterExpression {
        self.value = value.into();
        self.operator = FilterOperator::Eq;
        FilterExpression::Text(self)
    }

    /// Create an inequality filter.
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = TextFilter::new("job").ne("engineer");
    /// // Generates: (-@job:"engineer")
    /// ```
    pub fn ne(mut self, value: impl Into<String>) -> FilterExpression {
        self.value = value.into();
        self.operator = FilterOperator::Ne;
        FilterExpression::Text(self)
    }

    /// Create a LIKE filter with wildcards.
    ///
    /// Supports:
    /// - Prefix match: "engine*"
    /// - Fuzzy match: "%%engine%%"
    /// - Multiple terms (OR): "engineer|doctor"
    /// - Multiple terms (AND): "engineer doctor"
    ///
    /// # Example
    /// ```rust,ignore
    /// let filter = TextFilter::new("job").like("engine*");
    /// // Generates: @job:(engine*)
    /// ```
    pub fn like(mut self, pattern: impl Into<String>) -> FilterExpression {
        self.value = pattern.into();
        self.operator = FilterOperator::Like;
        FilterExpression::Text(self)
    }

    /// Generate the Redis query string for this filter.
    fn to_query_string(&self) -> String {
        if self.value.is_empty() {
            return "*".to_string();
        }

        match self.operator {
            FilterOperator::Eq => format!("@{}:(\"{}\")", self.field, self.value),
            FilterOperator::Ne => format!("(-@{}:\"{}\")", self.field, self.value),
            FilterOperator::Like => format!("@{}:({})", self.field, self.value),
            _ => unreachable!("TextFilter only supports Eq, Ne, Like operators"),
        }
    }
}

/// Filter expression that can be combined with AND/OR operators.
///
/// `FilterExpressions` are built from `TagFilter`, `NumFilter`, and `TextFilter`,
/// and can be combined to create complex logical expressions.
#[derive(Debug, Clone)]
pub enum FilterExpression {
    /// Tag filter expression
    Tag(TagFilter),
    /// Numeric filter expression
    Num(NumFilter),
    /// Text filter expression
    Text(TextFilter),
    /// AND combination of two expressions
    And(Box<FilterExpression>, Box<FilterExpression>),
    /// OR combination of two expressions
    Or(Box<FilterExpression>, Box<FilterExpression>),
}

impl FilterExpression {
    /// Combine this expression with another using AND.
    ///
    /// # Example
    /// ```rust,ignore
    /// let brand = TagFilter::new("brand").eq("nike");
    /// let price = NumFilter::new("price").lt(100.0);
    /// let filter = brand.and(price);
    /// // Generates: (@brand:{nike} @price:[-inf (100])
    /// ```
    #[must_use]
    pub fn and(self, other: FilterExpression) -> FilterExpression {
        FilterExpression::And(Box::new(self), Box::new(other))
    }

    /// Combine this expression with another using OR.
    ///
    /// # Example
    /// ```rust,ignore
    /// let brand1 = TagFilter::new("brand").eq("nike");
    /// let brand2 = TagFilter::new("brand").eq("adidas");
    /// let filter = brand1.or(brand2);
    /// // Generates: (@brand:{nike} | @brand:{adidas})
    /// ```
    #[must_use]
    pub fn or(self, other: FilterExpression) -> FilterExpression {
        FilterExpression::Or(Box::new(self), Box::new(other))
    }

    /// Convert the expression to a Redis query string.
    #[must_use]
    pub fn to_query_string(&self) -> String {
        self.to_string()
    }
}

impl fmt::Display for FilterExpression {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FilterExpression::Tag(filter) => write!(f, "{}", filter.to_query_string()),
            FilterExpression::Num(filter) => write!(f, "{}", filter.to_query_string()),
            FilterExpression::Text(filter) => write!(f, "{}", filter.to_query_string()),
            FilterExpression::And(left, right) => {
                let left_str = left.to_string();
                let right_str = right.to_string();

                // Handle wildcard cases
                if left_str == "*" && right_str == "*" {
                    write!(f, "*")
                } else if left_str == "*" {
                    write!(f, "{right_str}")
                } else if right_str == "*" {
                    write!(f, "{left_str}")
                } else {
                    write!(f, "({left_str} {right_str})")
                }
            }
            FilterExpression::Or(left, right) => {
                let left_str = left.to_string();
                let right_str = right.to_string();

                // Handle wildcard cases
                if left_str == "*" && right_str == "*" {
                    write!(f, "*")
                } else if left_str == "*" {
                    write!(f, "{right_str}")
                } else if right_str == "*" {
                    write!(f, "{left_str}")
                } else {
                    write!(f, "({left_str} | {right_str})")
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_redis_value() {
        assert_eq!(escape_redis_value("simple"), "simple");
        assert_eq!(escape_redis_value("with,comma"), "with\\,comma");
        assert_eq!(escape_redis_value("with|pipe"), "with\\|pipe");
        assert_eq!(escape_redis_value("with{brace}"), "with\\{brace\\}");
    }

    // TagFilter tests
    #[test]
    fn test_tag_filter_eq() {
        let filter = TagFilter::new("brand").eq("nike");
        assert_eq!(filter.to_string(), "@brand:{nike}");
    }

    #[test]
    fn test_tag_filter_ne() {
        let filter = TagFilter::new("brand").ne("nike");
        assert_eq!(filter.to_string(), "(-@brand:{nike})");
    }

    #[test]
    fn test_tag_filter_in_values() {
        let filter = TagFilter::new("brand").in_values(vec!["nike", "adidas"]);
        assert_eq!(filter.to_string(), "@brand:{nike|adidas}");
    }

    #[test]
    fn test_tag_filter_in_values_single() {
        let filter = TagFilter::new("brand").in_values(vec!["nike"]);
        assert_eq!(filter.to_string(), "@brand:{nike}");
    }

    #[test]
    fn test_tag_filter_escapes_special_chars() {
        let filter = TagFilter::new("tag").eq("value,with|special{chars}");
        assert_eq!(
            filter.to_string(),
            "@tag:{value\\,with\\|special\\{chars\\}}"
        );
    }

    // NumFilter tests
    #[test]
    fn test_num_filter_eq() {
        let filter = NumFilter::new("price").eq(99.99);
        assert_eq!(filter.to_string(), "@price:[99.99 99.99]");
    }

    #[test]
    fn test_num_filter_ne() {
        let filter = NumFilter::new("price").ne(99.99);
        assert_eq!(filter.to_string(), "(-@price:[99.99 99.99])");
    }

    #[test]
    fn test_num_filter_lt() {
        let filter = NumFilter::new("price").lt(100.0);
        assert_eq!(filter.to_string(), "@price:[-inf (100]");
    }

    #[test]
    fn test_num_filter_gt() {
        let filter = NumFilter::new("price").gt(100.0);
        assert_eq!(filter.to_string(), "@price:[(100 +inf]");
    }

    #[test]
    fn test_num_filter_le() {
        let filter = NumFilter::new("price").le(100.0);
        assert_eq!(filter.to_string(), "@price:[-inf 100]");
    }

    #[test]
    fn test_num_filter_ge() {
        let filter = NumFilter::new("price").ge(100.0);
        assert_eq!(filter.to_string(), "@price:[100 +inf]");
    }

    #[test]
    fn test_num_filter_negative_values() {
        let filter = NumFilter::new("temp").lt(-10.5);
        assert_eq!(filter.to_string(), "@temp:[-inf (-10.5]");
    }

    // TextFilter tests
    #[test]
    fn test_text_filter_eq() {
        let filter = TextFilter::new("job").eq("engineer");
        assert_eq!(filter.to_string(), "@job:(\"engineer\")");
    }

    #[test]
    fn test_text_filter_ne() {
        let filter = TextFilter::new("job").ne("engineer");
        assert_eq!(filter.to_string(), "(-@job:\"engineer\")");
    }

    #[test]
    fn test_text_filter_like_wildcard() {
        let filter = TextFilter::new("job").like("engine*");
        assert_eq!(filter.to_string(), "@job:(engine*)");
    }

    #[test]
    fn test_text_filter_like_fuzzy() {
        let filter = TextFilter::new("job").like("%%engineer%%");
        assert_eq!(filter.to_string(), "@job:(%%engineer%%)");
    }

    #[test]
    fn test_text_filter_like_multiple_or() {
        let filter = TextFilter::new("job").like("engineer|doctor");
        assert_eq!(filter.to_string(), "@job:(engineer|doctor)");
    }

    #[test]
    fn test_text_filter_like_multiple_and() {
        let filter = TextFilter::new("job").like("engineer doctor");
        assert_eq!(filter.to_string(), "@job:(engineer doctor)");
    }

    // FilterExpression combination tests
    #[test]
    fn test_filter_and_combination() {
        let brand = TagFilter::new("brand").eq("nike");
        let price = NumFilter::new("price").lt(100.0);
        let filter = brand.and(price);
        assert_eq!(filter.to_string(), "(@brand:{nike} @price:[-inf (100])");
    }

    #[test]
    fn test_filter_or_combination() {
        let brand1 = TagFilter::new("brand").eq("nike");
        let brand2 = TagFilter::new("brand").eq("adidas");
        let filter = brand1.or(brand2);
        assert_eq!(filter.to_string(), "(@brand:{nike} | @brand:{adidas})");
    }

    #[test]
    fn test_filter_complex_combination() {
        // (brand == nike AND price < 100) OR (brand == adidas AND price < 50)
        let nike_cheap = TagFilter::new("brand")
            .eq("nike")
            .and(NumFilter::new("price").lt(100.0));

        let adidas_cheaper = TagFilter::new("brand")
            .eq("adidas")
            .and(NumFilter::new("price").lt(50.0));

        let filter = nike_cheap.or(adidas_cheaper);
        assert_eq!(
            filter.to_string(),
            "((@brand:{nike} @price:[-inf (100]) | (@brand:{adidas} @price:[-inf (50]))"
        );
    }

    #[test]
    fn test_filter_three_way_and() {
        let brand = TagFilter::new("brand").eq("nike");
        let price = NumFilter::new("price").lt(100.0);
        let rating = NumFilter::new("rating").ge(4.0);
        let filter = brand.and(price).and(rating);
        assert_eq!(
            filter.to_string(),
            "((@brand:{nike} @price:[-inf (100]) @rating:[4 +inf])"
        );
    }

    #[test]
    fn test_filter_three_way_or() {
        let brand1 = TagFilter::new("brand").eq("nike");
        let brand2 = TagFilter::new("brand").eq("adidas");
        let brand3 = TagFilter::new("brand").eq("puma");
        let filter = brand1.or(brand2).or(brand3);
        assert_eq!(
            filter.to_string(),
            "((@brand:{nike} | @brand:{adidas}) | @brand:{puma})"
        );
    }

    #[test]
    fn test_filter_mixed_types() {
        let tag = TagFilter::new("category").eq("electronics");
        let num = NumFilter::new("price").ge(50.0);
        let text = TextFilter::new("description").like("wireless*");
        let filter = tag.and(num).and(text);
        assert_eq!(
            filter.to_string(),
            "((@category:{electronics} @price:[50 +inf]) @description:(wireless*))"
        );
    }

    #[test]
    fn test_filter_to_query_string_method() {
        let filter = TagFilter::new("brand").eq("nike");
        assert_eq!(filter.to_query_string(), "@brand:{nike}");
    }

    #[test]
    fn test_num_filter_integer_values() {
        let filter = NumFilter::new("age").ge(18.0);
        assert_eq!(filter.to_string(), "@age:[18 +inf]");
    }

    #[test]
    fn test_multiple_tags_in_filter() {
        let filter = TagFilter::new("colors").in_values(vec!["red", "blue", "green"]);
        assert_eq!(filter.to_string(), "@colors:{red|blue|green}");
    }

    #[test]
    fn test_text_filter_with_spaces() {
        let filter = TextFilter::new("title").eq("hello world");
        assert_eq!(filter.to_string(), "@title:(\"hello world\")");
    }

    #[test]
    fn test_text_filter_empty_string() {
        let filter = TextFilter::new("title").eq("");
        assert_eq!(filter.to_string(), "*");
    }
}
