use chrono::{Local, Duration, Months};
use regex::Regex;
use std::collections::HashMap;

/// Template processor that handles Templater-style expressions
/// Supports date expressions and numeric calculations
pub struct TemplateProcessor;

/// Types of expressions that can be processed
#[derive(Debug, Clone, PartialEq)]
enum ExpressionType {
    DateExpression {
        format: String,
        offset: Option<String>,
    },
    NumericExpression(String),
    SimpleVariable(String),
}

impl TemplateProcessor {
    /// Process a template string, replacing all expressions
    /// Processing order:
    /// 1. Date expressions ({{date:FORMAT| OFFSET}})
    /// 2. Numeric calculations ({{2 + 3}})
    /// 3. Simple variable substitution ({{variable}})
    pub fn process(template: &str, variables: &HashMap<String, String>) -> String {
        // Step 1: Find all expressions that need processing
        let expressions = Self::find_expressions(template);
        
        // Step 2: Process each expression type
        let mut result = template.to_string();
        
        // Process date expressions first
        for expr in &expressions {
            if let ExpressionType::DateExpression { format, offset } = expr {
                if let Ok(replacement) = Self::evaluate_date_expression(format, offset.as_deref()) {
                    let pattern = Self::build_date_pattern(format, offset.as_deref());
                    result = result.replace(&pattern, &replacement);
                }
                // If evaluation fails, leave as-is (graceful failure)
            }
        }
        
        // Process numeric expressions
        for expr in &expressions {
            if let ExpressionType::NumericExpression(expr_str) = expr {
                if let Ok(replacement) = Self::evaluate_numeric_expression(expr_str) {
                    let pattern = format!("{{{{{}}}}}", expr_str);
                    result = result.replace(&pattern, &replacement);
                }
                // If evaluation fails, leave as-is
            }
        }
        
        // Step 3: Simple variable substitution (only for remaining {{variable}} patterns)
        for (key, value) in variables {
            let placeholder = format!("{{{{{}}}}}", key);
            // Only replace if it's a simple variable (not already processed)
            if !Self::is_processed_expression(&placeholder) {
                result = result.replace(&placeholder, value);
            }
        }
        
        result
    }
    
    /// Find all expressions in the template that need processing
    fn find_expressions(template: &str) -> Vec<ExpressionType> {
        let mut expressions = Vec::new();
        
        // Regex to find all {{...}} patterns
        let expr_regex = Regex::new(r"\{\{([^}]+)\}\}").unwrap();
        
        for cap in expr_regex.captures_iter(template) {
            if let Some(expr_content) = cap.get(1) {
                let expr_str = expr_content.as_str().trim();
                
                // Check if it's a date expression
                if let Some(date_expr) = Self::parse_date_expression(expr_str) {
                    expressions.push(date_expr);
                }
                // Check if it's a numeric expression
                else if Self::is_numeric_expression(expr_str) {
                    expressions.push(ExpressionType::NumericExpression(expr_str.to_string()));
                }
                // Otherwise, it's a simple variable (will be handled in variable substitution)
            }
        }
        
        expressions
    }
    
    /// Parse a date expression like "date:YYYY-MM-DD| -7d"
    fn parse_date_expression(expr: &str) -> Option<ExpressionType> {
        if !expr.starts_with("date:") {
            return None;
        }
        
        let rest = &expr[5..]; // Skip "date:"
        
        // Split by "|" to separate format and offset
        let parts: Vec<&str> = rest.split('|').map(|s| s.trim()).collect();
        
        if parts.is_empty() {
            return None;
        }
        
        let format = parts[0].to_string();
        let offset = parts.get(1).map(|s| s.to_string());
        
        Some(ExpressionType::DateExpression { format, offset })
    }
    
    /// Build the full pattern for a date expression
    fn build_date_pattern(format: &str, offset: Option<&str>) -> String {
        if let Some(off) = offset {
            format!("{{{{date:{}| {}}}}}", format, off)
        } else {
            format!("{{{{date:{}}}}}", format)
        }
    }
    
    /// Evaluate a date expression
    fn evaluate_date_expression(format: &str, offset: Option<&str>) -> Result<String, String> {
        // Get base date (now)
        let mut date = Local::now();
        
        // Apply offset if present
        if let Some(off) = offset {
            date = Self::apply_date_offset(date, off)?;
        }
        
        // Convert moment.js format to chrono format and format the date
        let chrono_format = Self::moment_to_chrono_format(format)?;
        Ok(date.format(&chrono_format).to_string())
    }
    
    /// Apply date offset like "-7d", "+1w", "-2m", "+1y"
    fn apply_date_offset(date: chrono::DateTime<Local>, offset: &str) -> Result<chrono::DateTime<Local>, String> {
        let offset = offset.trim();
        if offset.is_empty() {
            return Ok(date);
        }
        
        // Parse offset: [+-]?[0-9]+[dwmy]
        let offset_regex = Regex::new(r"([+-]?)(\d+)([dwmy])").unwrap();
        
        let mut result_date = date;
        
        for cap in offset_regex.captures_iter(offset) {
            let sign = cap.get(1).map(|m| m.as_str()).unwrap_or("+");
            let amount: i64 = cap.get(2)
                .and_then(|m| m.as_str().parse().ok())
                .ok_or_else(|| format!("Invalid offset amount in: {}", offset))?;
            let unit = cap.get(3)
                .and_then(|m| m.as_str().chars().next())
                .ok_or_else(|| format!("Invalid offset unit in: {}", offset))?;
            
            let actual_amount = if sign == "-" { -amount } else { amount };
            
            match unit {
                'd' => {
                    result_date = result_date + Duration::days(actual_amount);
                }
                'w' => {
                    result_date = result_date + Duration::weeks(actual_amount);
                }
                'm' => {
                    // Months need special handling - chrono supports both positive and negative
                    if actual_amount >= 0 {
                        let months = Months::new(actual_amount as u32);
                        result_date = result_date.checked_add_months(months)
                            .ok_or_else(|| format!("Invalid month offset: {}", actual_amount))?;
                    } else {
                        // For negative, we need to subtract
                        let months = Months::new((-actual_amount) as u32);
                        result_date = result_date.checked_sub_months(months)
                            .ok_or_else(|| format!("Invalid month offset: {}", actual_amount))?;
                    }
                }
                'y' => {
                    // Years as months
                    if actual_amount >= 0 {
                        let months = Months::new((actual_amount * 12) as u32);
                        result_date = result_date.checked_add_months(months)
                            .ok_or_else(|| format!("Invalid year offset: {}", actual_amount))?;
                    } else {
                        let months = Months::new(((-actual_amount) * 12) as u32);
                        result_date = result_date.checked_sub_months(months)
                            .ok_or_else(|| format!("Invalid year offset: {}", actual_amount))?;
                    }
                }
                _ => return Err(format!("Unknown offset unit: {}", unit)),
            }
        }
        
        Ok(result_date)
    }
    
    /// Convert moment.js format string to chrono format string
    /// Supports: YYYY, MM, DD, HH, mm, ss, ww, ddd, dddd, MMM, MMMM, etc.
    fn moment_to_chrono_format(moment_format: &str) -> Result<String, String> {
        let mut result = String::new();
        let mut chars = moment_format.chars().peekable();
        
        while let Some(ch) = chars.next() {
            match ch {
                'Y' => {
                    // YYYY = 4-digit year, YY = 2-digit year
                    let mut count = 1;
                    while chars.peek() == Some(&'Y') {
                        chars.next();
                        count += 1;
                    }
                    if count >= 4 {
                        result.push_str("%Y");
                    } else {
                        result.push_str("%y");
                    }
                }
                'M' => {
                    // MMMM = full month name, MMM = abbreviated, MM = zero-padded, M = unpadded
                    let mut count = 1;
                    while chars.peek() == Some(&'M') {
                        chars.next();
                        count += 1;
                    }
                    match count {
                        4 => result.push_str("%B"),  // Full month name
                        3 => result.push_str("%b"),  // Abbreviated month
                        2 => result.push_str("%m"),  // Zero-padded month
                        _ => result.push_str("%-m"), // Unpadded month (may not work in all chrono versions)
                    }
                }
                'D' => {
                    // DDD = day of year, DD = zero-padded day, D = unpadded
                    let mut count = 1;
                    while chars.peek() == Some(&'D') {
                        chars.next();
                        count += 1;
                    }
                    match count {
                        3 => result.push_str("%j"),  // Day of year
                        2 => result.push_str("%d"),  // Zero-padded day
                        _ => result.push_str("%-d"),  // Unpadded day
                    }
                }
                'H' => {
                    // HH = 24-hour zero-padded, H = unpadded
                    let mut count = 1;
                    while chars.peek() == Some(&'H') {
                        chars.next();
                        count += 1;
                    }
                    if count >= 2 {
                        result.push_str("%H");
                    } else {
                        result.push_str("%-H");
                    }
                }
                'm' => {
                    // mm = minutes zero-padded, m = unpadded
                    let mut count = 1;
                    while chars.peek() == Some(&'m') {
                        chars.next();
                        count += 1;
                    }
                    if count >= 2 {
                        result.push_str("%M");
                    } else {
                        result.push_str("%-M");
                    }
                }
                's' => {
                    // ss = seconds zero-padded, s = unpadded
                    let mut count = 1;
                    while chars.peek() == Some(&'s') {
                        chars.next();
                        count += 1;
                    }
                    if count >= 2 {
                        result.push_str("%S");
                    } else {
                        result.push_str("%-S");
                    }
                }
                'w' => {
                    // ww = ISO week number, w = week number
                    let mut count = 1;
                    while chars.peek() == Some(&'w') {
                        chars.next();
                        count += 1;
                    }
                    if count >= 2 {
                        result.push_str("%V"); // ISO week number
                    } else {
                        result.push_str("%V"); // Same for now
                    }
                }
                'd' => {
                    // dddd = full day name, ddd = abbreviated, dd = zero-padded day, d = day of month
                    let mut count = 1;
                    while chars.peek() == Some(&'d') {
                        chars.next();
                        count += 1;
                    }
                    match count {
                        4 => result.push_str("%A"),  // Full day name
                        3 => result.push_str("%a"),  // Abbreviated day name
                        2 => result.push_str("%d"),  // Zero-padded day
                        _ => result.push_str("%-d"), // Unpadded day
                    }
                }
                'a' => {
                    // a = am/pm
                    if chars.peek() == Some(&'a') {
                        chars.next();
                        result.push_str("%p"); // AM/PM
                    } else {
                        result.push(ch);
                    }
                }
                'A' => {
                    // A = AM/PM (same as 'a')
                    result.push_str("%p");
                }
                _ => {
                    // Literal character - escape if needed
                    if ch.is_alphanumeric() {
                        result.push(ch);
                    } else {
                        // Escape special characters
                        result.push(ch);
                    }
                }
            }
        }
        
        Ok(result)
    }
    
    /// Check if an expression is a numeric calculation
    fn is_numeric_expression(expr: &str) -> bool {
        // Simple check: contains math operators and numbers
        // This is a basic implementation - can be extended
        let has_operator = expr.contains('+') || expr.contains('-') || 
                          expr.contains('*') || expr.contains('/') ||
                          expr.contains('%');
        let has_number = expr.chars().any(|c| c.is_ascii_digit());
        has_operator && has_number
    }
    
    /// Evaluate a numeric expression (basic math)
    fn evaluate_numeric_expression(expr: &str) -> Result<String, String> {
        // Very basic implementation - just for simple arithmetic
        // For production, consider using a proper expression evaluator
        
        // Remove whitespace
        let expr = expr.replace(' ', "");
        
        // Try to parse and evaluate simple expressions
        // This is a simplified evaluator - handles: number op number
        let re = Regex::new(r"(-?\d+\.?\d*)\s*([+\-*/%])\s*(-?\d+\.?\d*)").unwrap();
        
        if let Some(cap) = re.captures(&expr) {
            let left: f64 = cap.get(1).unwrap().as_str().parse()
                .map_err(|_| "Invalid number")?;
            let op = cap.get(2).unwrap().as_str();
            let right: f64 = cap.get(3).unwrap().as_str().parse()
                .map_err(|_| "Invalid number")?;
            
            let result = match op {
                "+" => left + right,
                "-" => left - right,
                "*" => left * right,
                "/" => {
                    if right == 0.0 {
                        return Err("Division by zero".to_string());
                    }
                    left / right
                }
                "%" => ((left as i64) % (right as i64)) as f64,
                _ => return Err(format!("Unknown operator: {}", op)),
            };
            
            // Format result - remove .0 for integers
            if result.fract() == 0.0 {
                Ok(result as i64 as i32).map(|n| n.to_string())
            } else {
                Ok(result.to_string())
            }
        } else {
            Err("Expression not supported".to_string())
        }
    }
    
    /// Check if a placeholder was already processed (to avoid double substitution)
    fn is_processed_expression(placeholder: &str) -> bool {
        placeholder.contains("date:") || placeholder.contains('+') || 
        placeholder.contains('-') || placeholder.contains('*') || 
        placeholder.contains('/') || placeholder.contains('%')
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_date_expression_parsing() {
        let expr = "date:YYYY-MM-DD| -7d";
        let result = TemplateProcessor::parse_date_expression(expr);
        assert!(matches!(result, Some(ExpressionType::DateExpression { .. })));
    }
    
    #[test]
    fn test_numeric_expression() {
        let result = TemplateProcessor::evaluate_numeric_expression("2 + 3");
        assert_eq!(result, Ok("5".to_string()));
    }
}

