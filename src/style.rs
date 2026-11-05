use crate::css::{CssColor, CssParser, Rule, Selector, SelectorPart, SimpleSelector, Specificity, Stylesheet, Unit, Value};
use crate::dom::{Node, NodeType};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Inline,
    Block,
    InlineBlock,
    Flex,
    Grid,
    None,
}

#[derive(Debug, Clone)]
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub specified_values: HashMap<String, Value>,
    pub children: Vec<StyledNode<'a>>,
    pub link_url: Option<String>,
}

pub type MatchedRule<'a> = (Specificity, &'a Rule);

impl<'a> StyledNode<'a> {
    pub fn value(&self, name: &str) -> Option<&Value> {
        self.specified_values.get(name)
    }

    pub fn display(&self) -> Display {
        if let Some(tag) = self.node.tag_name() {
            if tag == "html" || tag == "body" {
                return Display::Block;
            }
            if matches!(
                tag,
                "head"
                    | "meta"
                    | "link"
                    | "title"
                    | "style"
                    | "script"
                    | "noscript"
                    | "template"
            ) {
                return Display::None;
            }
        }

        // HTML5 hidden attribute defaults to display: none unless overridden
        if self.node.get_attribute("hidden").is_some() && self.value("display").is_none() {
            return Display::None;
        }

        let specified_display = match self.value("display") {
            Some(Value::Keyword(s)) => match s.as_str() {
                "block" => Display::Block,
                "none" => Display::None,
                "inline-block" => Display::InlineBlock,
                "flex" => Display::Flex,
                "grid" => Display::Grid,
                "inline-flex" | "inline-grid" => Display::InlineBlock,
                _ => Display::Inline,
            },
            _ => {
                if let Some(tag) = self.node.tag_name() {
                    if is_default_block_element(tag) {
                        Display::Block
                    } else {
                        Display::Inline
                    }
                } else {
                    Display::Inline
                }
            }
        };

        if specified_display == Display::None {
            return Display::None;
        }

        // CSS 2.1 Section 9.7: position: absolute or fixed computes inline to block
        if let Some(Value::Keyword(pos)) = self.value("position") {
            if pos == "absolute" || pos == "fixed" {
                if specified_display == Display::Inline || specified_display == Display::InlineBlock {
                    return Display::Block;
                }
            }
        }

        specified_display
    }

    /// Looks up a property; if not found, checks if it is an inheritable property
    /// and retrieves it from `parent`.
    pub fn lookup(&self, name: &str, fallback: &Value, parent: Option<&StyledNode>) -> Value {
        if let Some(v) = self.value(name) {
            return v.clone();
        }

        if is_inheritable(name) {
            if let Some(p) = parent {
                return p.lookup(name, fallback, None);
            }
        }

        fallback.clone()
    }

    pub fn color(&self, parent: Option<&StyledNode>) -> CssColor {
        let fallback = Value::ColorValue(CssColor::new(32, 33, 36, 255));
        match self.lookup("color", &fallback, parent) {
            Value::ColorValue(c) => c,
            _ => CssColor::new(32, 33, 36, 255),
        }
    }

    pub fn background_color(&self) -> CssColor {
        match self.value("background-color") {
            Some(Value::ColorValue(c)) => *c,
            _ => CssColor::TRANSPARENT,
        }
    }

    pub fn font_size(&self, parent: Option<&StyledNode>) -> f32 {
        let fallback = Value::Length(16.0, Unit::Px);
        match self.lookup("font-size", &fallback, parent) {
            Value::Length(px, Unit::Px) => px,
            Value::Length(pt, Unit::Pt) => pt * 1.333,
            Value::Length(em, Unit::Em) => {
                let parent_size = parent.map(|p| p.font_size(None)).unwrap_or(16.0);
                em * parent_size
            }
            Value::Length(rem, Unit::Rem) => rem * 16.0,
            Value::Length(pct, Unit::Percent) => {
                let parent_size = parent.map(|p| p.font_size(None)).unwrap_or(16.0);
                parent_size * (pct / 100.0)
            }
            Value::Keyword(k) => match k.as_str() {
                "xx-small" => 9.0,
                "x-small" => 10.0,
                "small" => 13.0,
                "medium" => 16.0,
                "large" => 18.0,
                "x-large" => 24.0,
                "xx-large" => 32.0,
                "smaller" => parent.map(|p| p.font_size(None)).unwrap_or(16.0) * 0.85,
                "larger" => parent.map(|p| p.font_size(None)).unwrap_or(16.0) * 1.2,
                _ => 16.0,
            },
            _ => 16.0,
        }
    }

    pub fn line_height(&self, font_size: f32, parent: Option<&StyledNode>) -> f32 {
        let fallback = Value::Number(1.35);
        match self.lookup("line-height", &fallback, parent) {
            Value::Number(n) => (font_size * n).max(8.0),
            Value::Length(px, Unit::Px) => px.max(8.0),
            Value::Length(em, Unit::Em) => (font_size * em).max(8.0),
            Value::Length(rem, Unit::Rem) => (16.0 * rem).max(8.0),
            Value::Length(pct, Unit::Percent) => (font_size * (pct / 100.0)).max(8.0),
            Value::Keyword(k) if k == "normal" => font_size * 1.35,
            _ => font_size * 1.35,
        }
    }

    pub fn is_bold(&self, parent: Option<&StyledNode>) -> bool {
        let fallback = Value::Keyword("normal".to_string());
        match self.lookup("font-weight", &fallback, parent) {
            Value::Keyword(ref s) => s == "bold" || s == "bolder" || s == "700" || s == "800" || s == "900",
            Value::Number(n) => n >= 600.0,
            _ => false,
        }
    }

    #[allow(dead_code)]
    pub fn is_italic(&self, parent: Option<&StyledNode>) -> bool {
        let fallback = Value::Keyword("normal".to_string());
        match self.lookup("font-style", &fallback, parent) {
            Value::Keyword(ref s) => s == "italic" || s == "oblique",
            _ => false,
        }
    }
}

fn is_default_block_element(tag: &str) -> bool {
    matches!(
        tag,
        "html"
            | "body"
            | "div"
            | "p"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "section"
            | "article"
            | "nav"
            | "header"
            | "footer"
            | "main"
            | "form"
            | "ul"
            | "ol"
            | "li"
            | "pre"
            | "blockquote"
            | "hr"
            | "table"
            | "tr"
    )
}

fn is_inheritable(property: &str) -> bool {
    property.starts_with("--")
        || matches!(
            property,
            "color"
                | "font-size"
                | "font-weight"
                | "font-style"
                | "font-family"
                | "line-height"
                | "text-align"
                | "visibility"
        )
}

fn resolve_var_value(val_str: &str, values: &HashMap<String, Value>, depth: usize) -> Option<Value> {
    if depth > 10 {
        return None;
    }
    let inner = val_str.strip_prefix("var(")?.strip_suffix(')')?.trim();
    let (var_name, fallback) = if let Some(comma_pos) = inner.find(',') {
        (inner[..comma_pos].trim(), Some(inner[comma_pos + 1..].trim()))
    } else {
        (inner, None)
    };

    if let Some(v) = values.get(var_name) {
        match v {
            Value::Keyword(k) if k.starts_with("var(") && k.ends_with(')') => {
                resolve_var_value(k, values, depth + 1)
            }
            _ => Some(v.clone()),
        }
    } else if let Some(fb) = fallback {
        if fb.starts_with("var(") && fb.ends_with(')') {
            resolve_var_value(fb, values, depth + 1)
        } else {
            crate::css::parse_value_str(fb)
        }
    } else {
        None
    }
}

/// Builds a styled tree from a root DOM node, an optional parent hierarchy, and stylesheets.
pub fn style_tree<'a>(
    root: &'a Node,
    stylesheet: &Stylesheet,
) -> StyledNode<'a> {
    let mut ancestors = Vec::new();
    build_styled_node(root, &mut ancestors, stylesheet, None)
}

fn build_styled_node<'a>(
    node: &'a Node,
    ancestors: &mut Vec<&'a Node>,
    stylesheet: &Stylesheet,
    parent_styled: Option<&StyledNode>,
) -> StyledNode<'a> {
    let mut specified_values = match &node.node_type {
        NodeType::Element(_) => specified_values(node, ancestors, stylesheet),
        NodeType::Text(_) => {
            // Text nodes inherit typography from parent
            let mut values = HashMap::new();
            if let Some(parent) = parent_styled {
                for prop in ["color", "font-size", "font-weight", "font-style", "line-height"] {
                    if let Some(val) = parent.value(prop) {
                        values.insert(prop.to_string(), val.clone());
                    }
                }
            }
            values
        }
        NodeType::Comment(_) => HashMap::new(),
    };

    // Inherit inheritable properties and custom CSS properties (--*) from parent
    if let Some(parent) = parent_styled {
        for (prop, val) in &parent.specified_values {
            if is_inheritable(prop) && !specified_values.contains_key(prop) {
                specified_values.insert(prop.clone(), val.clone());
            }
        }
    }

    // Resolve any CSS variables (var(--...))
    let var_keys: Vec<String> = specified_values
        .iter()
        .filter_map(|(k, v)| match v {
            Value::Keyword(s) if s.starts_with("var(") && s.ends_with(')') => Some(k.clone()),
            _ => None,
        })
        .collect();

    for k in var_keys {
        if let Some(Value::Keyword(raw_var)) = specified_values.get(&k).cloned() {
            if let Some(resolved) = resolve_var_value(&raw_var, &specified_values, 0) {
                specified_values.insert(k, resolved);
            }
        }
    }

    let link_url = if node.tag_name() == Some("a") {
        node.get_attribute("href").map(|s| s.to_string())
    } else {
        parent_styled.and_then(|p| p.link_url.clone())
    };

    ancestors.push(node);

    // Create current styled node stub for passing as parent to children
    let current_stub = StyledNode {
        node,
        specified_values: specified_values.clone(),
        children: Vec::new(),
        link_url: link_url.clone(),
    };

    let children = node
        .children
        .iter()
        .map(|child| build_styled_node(child, ancestors, stylesheet, Some(&current_stub)))
        .collect();

    ancestors.pop();

    StyledNode {
        node,
        specified_values,
        children,
        link_url,
    }
}

fn specified_values(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &Stylesheet,
) -> HashMap<String, Value> {
    let mut values = HashMap::new();
    let mut important_keys = std::collections::HashSet::new();
    let mut rules = matching_rules(node, ancestors, stylesheet);

    // Sort by specificity: lowest first so higher specificity overrides
    rules.sort_by_key(|&(a, _)| a);

    for (_, rule) in rules {
        for declaration in &rule.declarations {
            if declaration.important {
                values.insert(declaration.name.clone(), declaration.value.clone());
                important_keys.insert(declaration.name.clone());
            } else if !important_keys.contains(&declaration.name) {
                values.insert(declaration.name.clone(), declaration.value.clone());
            }
        }
    }

    // Apply inline style="..." attribute with highest specificity
    if let Some(inline_css) = node.get_attribute("style") {
        let dummy_css = format!("* {{ {} }}", inline_css);
        let parsed_sheet = CssParser::parse(&dummy_css);
        for rule in parsed_sheet.rules {
            for declaration in rule.declarations {
                if declaration.important || !important_keys.contains(&declaration.name) {
                    values.insert(declaration.name.clone(), declaration.value);
                    if declaration.important {
                        important_keys.insert(declaration.name);
                    }
                }
            }
        }
    }

    values
}

fn matching_rules<'a>(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &'a Stylesheet,
) -> Vec<MatchedRule<'a>> {
    stylesheet
        .rules
        .iter()
        .filter_map(|rule| match_rule(node, ancestors, rule))
        .collect()
}

/// DEVTOOLS: Returns all CSS rules matching the given node within the stylesheet.
pub fn get_matched_rules<'a>(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &'a Stylesheet,
) -> Vec<MatchedRule<'a>> {
    matching_rules(node, ancestors, stylesheet)
}

/// DEVTOOLS: Computes the specified style property map for a given node.
pub fn get_specified_values(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &Stylesheet,
) -> HashMap<String, Value> {
    specified_values(node, ancestors, stylesheet)
}

fn match_rule<'a>(
    node: &Node,
    ancestors: &[&Node],
    rule: &'a Rule,
) -> Option<MatchedRule<'a>> {
    rule.selectors
        .iter()
        .find(|selector| matches_selector(node, ancestors, selector))
        .map(|selector| (selector.specificity(), rule))
}

fn matches_selector(node: &Node, ancestors: &[&Node], selector: &Selector) -> bool {
    if selector.parts.is_empty() {
        return false;
    }

    let mut part_idx = selector.parts.len() - 1;

    // The right-most part must match the current node
    match &selector.parts[part_idx] {
        SelectorPart::Simple(simple) => {
            if !matches_simple_selector(node, ancestors, simple) {
                return false;
            }
        }
        _ => return false,
    }

    if part_idx == 0 {
        return true;
    }

    // Work backwards through combinators and ancestors
    let mut current_ancestor_idx = ancestors.len();
    let mut current_node = node;

    while part_idx > 0 {
        part_idx -= 1;
        let combinator = &selector.parts[part_idx];
        if part_idx == 0 {
            return false;
        }
        part_idx -= 1;

        let target_simple = match &selector.parts[part_idx] {
            SelectorPart::Simple(s) => s,
            _ => return false,
        };

        match combinator {
            SelectorPart::ChildCombinator => {
                if current_ancestor_idx == 0 {
                    return false;
                }
                current_ancestor_idx -= 1;
                current_node = ancestors[current_ancestor_idx];
                if !matches_simple_selector(current_node, &ancestors[..current_ancestor_idx], target_simple) {
                    return false;
                }
            }
            SelectorPart::DescendantCombinator => {
                let mut found = false;
                while current_ancestor_idx > 0 {
                    current_ancestor_idx -= 1;
                    if matches_simple_selector(ancestors[current_ancestor_idx], &ancestors[..current_ancestor_idx], target_simple) {
                        current_node = ancestors[current_ancestor_idx];
                        found = true;
                        break;
                    }
                }
                if !found {
                    return false;
                }
            }
            SelectorPart::AdjacentSiblingCombinator => {
                let parent = if current_ancestor_idx > 0 {
                    ancestors[current_ancestor_idx - 1]
                } else {
                    return false;
                };
                let element_siblings: Vec<&Node> = parent
                    .children
                    .iter()
                    .filter(|c| matches!(c.node_type, NodeType::Element(_)))
                    .collect();
                let my_idx = element_siblings
                    .iter()
                    .position(|&s| std::ptr::eq(s, current_node));
                match my_idx {
                    Some(idx) if idx > 0 => {
                        let prev_sibling = element_siblings[idx - 1];
                        if !matches_simple_selector(prev_sibling, &ancestors[..current_ancestor_idx], target_simple) {
                            return false;
                        }
                        current_node = prev_sibling;
                    }
                    _ => return false,
                }
            }
            SelectorPart::GeneralSiblingCombinator => {
                let parent = if current_ancestor_idx > 0 {
                    ancestors[current_ancestor_idx - 1]
                } else {
                    return false;
                };
                let element_siblings: Vec<&Node> = parent
                    .children
                    .iter()
                    .filter(|c| matches!(c.node_type, NodeType::Element(_)))
                    .collect();
                let my_idx = element_siblings
                    .iter()
                    .position(|&s| std::ptr::eq(s, current_node));
                match my_idx {
                    Some(idx) if idx > 0 => {
                        let mut found = false;
                        for i in (0..idx).rev() {
                            let prev_sibling = element_siblings[i];
                            if matches_simple_selector(prev_sibling, &ancestors[..current_ancestor_idx], target_simple) {
                                current_node = prev_sibling;
                                found = true;
                                break;
                            }
                        }
                        if !found {
                            return false;
                        }
                    }
                    _ => return false,
                }
            }
            SelectorPart::Simple(_) => return false,
        }
    }

    true
}

fn matches_simple_selector(node: &Node, ancestors: &[&Node], selector: &SimpleSelector) -> bool {
    // Pseudo-elements (::before, ::after, etc.) never match the DOM element itself
    if selector.pseudo_element.is_some() {
        return false;
    }

    if !selector.universal {
        if let Some(ref tag) = selector.tag_name {
            if node.tag_name() != Some(tag) {
                return false;
            }
        }
    }

    if let Some(ref id) = selector.id {
        if node.id() != Some(id) {
            return false;
        }
    }

    let node_classes = node.classes();
    for class in &selector.classes {
        if !node_classes.contains(class.as_str()) {
            return false;
        }
    }

    for attr in &selector.attributes {
        match &attr.op {
            None => {
                if node.get_attribute(&attr.name).is_none() {
                    return false;
                }
            }
            Some(op) => {
                let node_val = match node.get_attribute(&attr.name) {
                    Some(v) => v,
                    None => return false,
                };
                let expected = attr.value.as_deref().unwrap_or("");
                let matched = match op.as_str() {
                    "=" => node_val == expected,
                    "~=" => node_val.split_whitespace().any(|w| w == expected),
                    "^=" => !expected.is_empty() && node_val.starts_with(expected),
                    "$=" => !expected.is_empty() && node_val.ends_with(expected),
                    "*=" => !expected.is_empty() && node_val.contains(expected),
                    "|=" => node_val == expected || node_val.starts_with(&format!("{}-", expected)),
                    _ => false,
                };
                if !matched {
                    return false;
                }
            }
        }
    }

    for pseudo in &selector.pseudo_classes {
        match pseudo.as_str() {
            "root" => {
                let is_root = node.tag_name() == Some("html") || ancestors.is_empty();
                if !is_root {
                    return false;
                }
            }
            "first-child" => {
                if let Some(parent) = ancestors.last() {
                    let first_elem = parent.children.iter().find(|c| matches!(c.node_type, NodeType::Element(_)));
                    if first_elem.map(|e| std::ptr::eq(e, node)) != Some(true) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            "last-child" => {
                if let Some(parent) = ancestors.last() {
                    let last_elem = parent.children.iter().rev().find(|c| matches!(c.node_type, NodeType::Element(_)));
                    if last_elem.map(|e| std::ptr::eq(e, node)) != Some(true) {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            "only-child" => {
                if let Some(parent) = ancestors.last() {
                    let elem_count = parent.children.iter().filter(|c| matches!(c.node_type, NodeType::Element(_))).count();
                    if elem_count != 1 {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            "empty" => {
                let has_elem_children = node.children.iter().any(|c| matches!(c.node_type, NodeType::Element(_)));
                if has_elem_children || !node.inner_text().trim().is_empty() {
                    return false;
                }
            }
            "checked" => {
                if node.get_attribute("checked").is_none() {
                    return false;
                }
            }
            "disabled" => {
                if node.get_attribute("disabled").is_none() {
                    return false;
                }
            }
            "enabled" => {
                if node.get_attribute("disabled").is_some() {
                    return false;
                }
            }
            "link" => {
                if node.tag_name() != Some("a") || node.get_attribute("href").is_none() {
                    return false;
                }
            }
            "hover" | "active" | "focus" | "focus-visible" | "focus-within" => {
                // Static page layout does not trigger active focus/hover states
                return false;
            }
            _ => {
                // Unknown pseudo-class defaults to not matching
                return false;
            }
        }
    }

    for not_sel in &selector.not_selectors {
        if matches_simple_selector(node, ancestors, not_sel) {
            return false;
        }
    }

    true
}
