use std::collections::{HashMap, HashSet};

/// Represents a node in the Document Object Model (DOM) tree.
#[derive(Debug, Clone)]
pub struct Node {
    pub children: Vec<Node>,
    pub node_type: NodeType,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType {
    Text(String),
    Element(ElementData),
    Comment(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ElementData {
    pub tag_name: String,
    pub attributes: HashMap<String, String>,
}

impl Node {
    pub fn new_text(data: String) -> Self {
        Node {
            children: Vec::new(),
            node_type: NodeType::Text(data),
        }
    }

    pub fn new_element(tag_name: String, attributes: HashMap<String, String>, children: Vec<Node>) -> Self {
        Node {
            children,
            node_type: NodeType::Element(ElementData {
                tag_name: tag_name.to_ascii_lowercase(),
                attributes,
            }),
        }
    }

    pub fn new_comment(data: String) -> Self {
        Node {
            children: Vec::new(),
            node_type: NodeType::Comment(data),
        }
    }

    /// Returns the element data if this node is an Element.
    pub fn element_data(&self) -> Option<&ElementData> {
        match &self.node_type {
            NodeType::Element(data) => Some(data),
            _ => None,
        }
    }

    /// Returns the tag name if this is an element.
    pub fn tag_name(&self) -> Option<&str> {
        self.element_data().map(|e| e.tag_name.as_str())
    }

    /// Returns the ID attribute of this element if present.
    pub fn id(&self) -> Option<&str> {
        self.element_data()?.attributes.get("id").map(|s| s.as_str())
    }

    /// Returns the set of classes declared on this element.
    pub fn classes(&self) -> HashSet<&str> {
        match self.element_data().and_then(|e| e.attributes.get("class")) {
            Some(class_str) => class_str.split_whitespace().collect(),
            None => HashSet::new(),
        }
    }

    /// Returns the value of an attribute if this node is an Element.
    pub fn get_attribute(&self, name: &str) -> Option<&str> {
        self.element_data()?.attributes.get(name).map(|s| s.as_str())
    }

    /// Recursively extracts all concatenated text from this node and its children.
    pub fn inner_text(&self) -> String {
        let mut text = String::new();
        self.collect_text(&mut text);
        text
    }

    fn collect_text(&self, out: &mut String) {
        match &self.node_type {
            NodeType::Text(s) => {
                out.push_str(s);
            }
            NodeType::Element(_) => {
                for child in &self.children {
                    child.collect_text(out);
                }
            }
            NodeType::Comment(_) => {}
        }
    }

    /// Searches for the first element matching `tag_name`.
    #[allow(dead_code)]
    pub fn find_first_by_tag(&self, tag: &str) -> Option<&Node> {
        let tag_lower = tag.to_ascii_lowercase();
        if self.tag_name() == Some(&tag_lower) {
            return Some(self);
        }
        for child in &self.children {
            if let Some(found) = child.find_first_by_tag(&tag_lower) {
                return Some(found);
            }
        }
        None
    }

    /// Recursively finds all elements matching `tag_name`.
    #[allow(dead_code)]
    pub fn find_all_by_tag<'a>(&'a self, tag: &str, out: &mut Vec<&'a Node>) {
        let tag_lower = tag.to_ascii_lowercase();
        if self.tag_name() == Some(&tag_lower) {
            out.push(self);
        }
        for child in &self.children {
            child.find_all_by_tag(&tag_lower, out);
        }
    }
}
