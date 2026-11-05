use crate::css::{CssColor, CssParser, Rule, Selector, SelectorPart, SimpleSelector, Specificity, Stylesheet, Unit, Value};
use crate::dom::{Node, NodeType};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Inline,
    Block,
    InlineBlock,
    None,
}

#[derive(Debug, Clone)]
pub struct StyledNode<'a> {
    pub node: &'a Node,
    pub specified_values: HashMap<String, Value>,
    pub children: Vec<StyledNode<'a>>,
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

        match self.value("display") {
            Some(Value::Keyword(s)) => match s.as_str() {
                "block" => Display::Block,
                "none" => Display::None,
                "inline-block" => Display::InlineBlock,
                _ => Display::Inline,
            },
            _ => {
                if let Some(tag) = self.node.tag_name() {
                    if is_default_block_element(tag) {
                        return Display::Block;
                    }
                }
                Display::Inline
            }
        }
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
            _ => 16.0,
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
    matches!(
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
    let specified_values = match &node.node_type {
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

    ancestors.push(node);

    // Create current styled node stub for passing as parent to children
    let current_stub = StyledNode {
        node,
        specified_values: specified_values.clone(),
        children: Vec::new(),
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
    }
}

fn specified_values(
    node: &Node,
    ancestors: &[&Node],
    stylesheet: &Stylesheet,
) -> HashMap<String, Value> {
    let mut values = HashMap::new();
    let mut rules = matching_rules(node, ancestors, stylesheet);

    // Sort by specificity: lowest first so higher specificity overrides
    rules.sort_by_key(|&(a, _)| a);

    for (_, rule) in rules {
        for declaration in &rule.declarations {
            values.insert(declaration.name.clone(), declaration.value.clone());
        }
    }

    // Apply inline style="..." attribute with highest specificity
    if let Some(inline_css) = node.get_attribute("style") {
        let dummy_css = format!("* {{ {} }}", inline_css);
        let parsed_sheet = CssParser::parse(&dummy_css);
        for rule in parsed_sheet.rules {
            for declaration in rule.declarations {
                values.insert(declaration.name, declaration.value);
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
            if !matches_simple_selector(node, simple) {
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
                // Must be immediate parent
                if current_ancestor_idx == 0 {
                    return false;
                }
                current_ancestor_idx -= 1;
                if !matches_simple_selector(ancestors[current_ancestor_idx], target_simple) {
                    return false;
                }
            }
            SelectorPart::DescendantCombinator => {
                // Any ancestor
                let mut found = false;
                while current_ancestor_idx > 0 {
                    current_ancestor_idx -= 1;
                    if matches_simple_selector(ancestors[current_ancestor_idx], target_simple) {
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

    true
}

fn matches_simple_selector(node: &Node, selector: &SimpleSelector) -> bool {
    if selector.universal {
        return true;
    }

    if let Some(ref tag) = selector.tag_name {
        if node.tag_name() != Some(tag) {
            return false;
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

    true
}
