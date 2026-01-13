//! Web page crawler for NatBot using Playwright.
//!
//! This module implements DOM extraction and element identification for browser automation.
//! It uses JavaScript evaluation to extract page state and identify interactive elements.

use dashflow::core::Error;
use dashflow_playwright::BrowserState;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Blacklisted element types that should not be included in the simplified DOM.
const BLACKLISTED_ELEMENTS: &[&str] = &[
    "html", "head", "title", "meta", "iframe", "body", "script", "style", "path", "svg", "br",
    "::marker",
];

/// JavaScript code for extracting DOM snapshot from the browser.
const EXTRACT_DOM_JS: &str = r#"
(() => {
    const strings = [];
    const stringMap = new Map();

    function addString(str) {
        if (stringMap.has(str)) {
            return stringMap.get(str);
        }
        const index = strings.length;
        strings.push(str);
        stringMap.set(str, index);
        return index;
    }

    const backendNodeId = [];
    const attributes = [];
    const nodeValue = [];
    const parentIndex = [];
    const nodeName = [];
    const isClickableIndex = [];
    const inputValueIndex = [];
    const inputValueValues = [];
    const layoutNodeIndex = [];
    const bounds = [];

    const walker = document.createTreeWalker(
        document.body,
        NodeFilter.SHOW_ELEMENT | NodeFilter.SHOW_TEXT,
        null,
        false
    );

    const nodeMap = new Map();
    let nodeIndex = 0;

    function processNode(node, parentIdx) {
        const idx = nodeIndex++;
        nodeMap.set(node, idx);

        // Backend node ID (use a hash or unique identifier)
        backendNodeId.push(idx);

        // Node name
        const name = node.nodeName.toLowerCase();
        nodeName.push(addString(name));

        // Parent index
        parentIndex.push(parentIdx);

        // Node value (for text nodes)
        if (node.nodeType === Node.TEXT_NODE) {
            nodeValue.push(addString(node.textContent.trim()));
        } else {
            nodeValue.push(-1);
        }

        // Attributes
        const attrs = [];
        if (node.attributes) {
            for (let i = 0; i < node.attributes.length; i++) {
                const attr = node.attributes[i];
                attrs.push(addString(attr.name));
                attrs.push(addString(attr.value));
            }
        }
        attributes.push(attrs);

        // Is clickable (check for click handlers, buttons, links, inputs)
        if (node.nodeType === Node.ELEMENT_NODE) {
            const clickable = node.onclick ||
                            name === "a" ||
                            name === "button" ||
                            name === "input" ||
                            node.hasAttribute("onclick");
            if (clickable) {
                isClickableIndex.push(idx);
            }

            // Input value
            if (name === "input" && node.value) {
                inputValueIndex.push(idx);
                inputValueValues.push(addString(node.value));
            }

            // Layout (bounds)
            const rect = node.getBoundingClientRect();
            if (rect.width > 0 || rect.height > 0) {
                layoutNodeIndex.push(idx);
                bounds.push([rect.x, rect.y, rect.width, rect.height]);
            }
        }

        // Process children
        for (let child of node.childNodes) {
            processNode(child, idx);
        }
    }

    processNode(document.body, -1);

    return {
        strings,
        document: {
            nodes: {
                backendNodeId,
                attributes,
                nodeValue,
                parentIndex,
                nodeName,
                isClickable: { index: isClickableIndex },
                inputValue: { index: inputValueIndex, value: inputValueValues }
            },
            layout: {
                nodeIndex: layoutNodeIndex,
                bounds
            }
        }
    };
})();
"#;

/// Information about an element in the viewport.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElementInViewPort {
    pub node_index: String,
    pub backend_node_id: i64,
    pub node_name: Option<String>,
    pub node_value: Option<String>,
    pub node_meta: Vec<String>,
    pub is_clickable: bool,
    pub origin_x: i32,
    pub origin_y: i32,
    pub center_x: i32,
    pub center_y: i32,
}

/// DOM snapshot data structures returned from JavaScript evaluation.
#[derive(Debug, Deserialize)]
struct DomSnapshot {
    strings: Vec<String>,
    document: Document,
}

#[derive(Debug, Deserialize)]
struct Document {
    nodes: Nodes,
    layout: Layout,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Nodes {
    backend_node_id: Vec<i64>,
    attributes: Vec<Vec<i32>>,
    node_value: Vec<i32>,
    parent_index: Vec<i32>,
    node_name: Vec<i32>,
    is_clickable: ClickableIndex,
    input_value: InputValue,
}

#[derive(Debug, Deserialize)]
struct ClickableIndex {
    index: Vec<usize>,
}

#[derive(Debug, Deserialize)]
struct InputValue {
    index: Vec<usize>,
    value: Vec<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Layout {
    node_index: Vec<usize>,
    bounds: Vec<Vec<f64>>,
}

/// A crawler for web pages using Playwright.
///
/// **Security Note**: This crawler can load arbitrary webpages INCLUDING content
/// from the local file system. Control access to who can submit crawling requests
/// and what network access the crawler has. Make sure to scope permissions to the
/// minimal permissions necessary for the application.
pub struct Crawler {
    browser_state: BrowserState,
    page_element_buffer: HashMap<i32, ElementInViewPort>,
}

impl Crawler {
    /// Create a new Crawler instance.
    pub async fn new() -> Result<Self, Error> {
        let browser_state = BrowserState::new().await?;
        Ok(Self {
            browser_state,
            page_element_buffer: HashMap::new(),
        })
    }

    /// Navigate to a URL.
    ///
    /// If the URL does not contain a scheme, it will be prefixed with "http://".
    pub async fn go_to_page(&mut self, url: &str) -> Result<(), Error> {
        let url = if url.contains("://") {
            url.to_string()
        } else {
            format!("http://{}", url)
        };
        self.browser_state.goto(&url).await?;
        self.page_element_buffer.clear();
        Ok(())
    }

    /// Scroll the page in the given direction.
    ///
    /// # Arguments
    /// * `direction` - Either "up" or "down"
    pub async fn scroll(&self, direction: &str) -> Result<(), Error> {
        self.browser_state.scroll(direction).await
    }

    /// Click on an element with the given id.
    ///
    /// # Arguments
    /// * `id` - The id of the element to click on
    pub async fn click(&self, id: i32) -> Result<(), Error> {
        // Remove target attribute from links to prevent new tabs
        let js = r#"
            var links = document.getElementsByTagName("a");
            for (var i = 0; i < links.length; i++) {
                links[i].removeAttribute("target");
            }
        "#;
        self.browser_state.eval::<()>(js).await?;

        if let Some(element) = self.page_element_buffer.get(&id) {
            let x = element.center_x as f64;
            let y = element.center_y as f64;
            self.browser_state.click_at(x, y).await?;
            Ok(())
        } else {
            Err(Error::tool_error(format!(
                "Could not find element with id {}",
                id
            )))
        }
    }

    /// Type text into an element with the given id.
    ///
    /// This will first click on the element, then type the text.
    pub async fn type_text(&self, id: i32, text: &str) -> Result<(), Error> {
        self.click(id).await?;
        self.browser_state.type_text(text).await
    }

    /// Press the Enter key.
    pub async fn enter(&self) -> Result<(), Error> {
        self.browser_state.press_key("Enter").await
    }

    /// Crawl the current page and return a simplified representation.
    ///
    /// Returns a list of HTML-like strings representing interactive elements.
    pub async fn crawl(&mut self) -> Result<Vec<String>, Error> {
        let start = std::time::Instant::now();

        // Get device pixel ratio
        let mut device_pixel_ratio: f64 = self
            .browser_state
            .eval("() => window.devicePixelRatio")
            .await?;

        // MacOS quirk: devicePixelRatio often reports 1 but is actually 2
        #[cfg(target_os = "macos")]
        if (device_pixel_ratio - 1.0).abs() < 0.01 {
            device_pixel_ratio = 2.0;
        }

        // Get viewport bounds
        let win_upper_bound: f64 = self.browser_state.eval("() => window.pageYOffset").await?;
        let win_left_bound: f64 = self.browser_state.eval("() => window.pageXOffset").await?;
        let win_width: f64 = self.browser_state.eval("() => window.screen.width").await?;
        let win_height: f64 = self
            .browser_state
            .eval("() => window.screen.height")
            .await?;
        let win_right_bound = win_left_bound + win_width;
        let win_lower_bound = win_upper_bound + win_height;

        // Extract DOM snapshot using JavaScript
        // This is a fallback approach since CDP may not be available in Rust playwright
        let snapshot = self.extract_dom_snapshot().await?;

        let strings = &snapshot.strings;
        let document = &snapshot.document;
        let nodes = &document.nodes;
        let backend_node_id = &nodes.backend_node_id;
        let attributes = &nodes.attributes;
        let node_value = &nodes.node_value;
        let parent = &nodes.parent_index;
        let node_names = &nodes.node_name;
        let is_clickable: HashSet<usize> = nodes.is_clickable.index.iter().copied().collect();

        let input_value_index = &nodes.input_value.index;
        let input_value_values = &nodes.input_value.value;

        let layout = &document.layout;
        let layout_node_index = &layout.node_index;
        let bounds = &layout.bounds;

        let mut child_nodes: HashMap<String, Vec<ChildNode>> = HashMap::new();
        let mut elements_in_view_port = Vec::new();

        let mut anchor_ancestry: HashMap<String, (bool, Option<usize>)> = HashMap::new();
        anchor_ancestry.insert("-1".to_string(), (false, None));
        let mut button_ancestry: HashMap<String, (bool, Option<usize>)> = HashMap::new();
        button_ancestry.insert("-1".to_string(), (false, None));

        // Process each node
        for (index, &node_name_index) in node_names.iter().enumerate() {
            let node_parent = parent[index];
            let node_name = strings[node_name_index as usize].to_lowercase();

            // Track anchor and button ancestry
            let (_is_ancestor_of_anchor, anchor_id) = Self::add_to_hash_tree(
                &mut anchor_ancestry,
                "a",
                index,
                &node_name,
                node_parent as usize,
                node_names,
                parent,
                strings,
            );

            let (_is_ancestor_of_button, button_id) = Self::add_to_hash_tree(
                &mut button_ancestry,
                "button",
                index,
                &node_name,
                node_parent as usize,
                node_names,
                parent,
                strings,
            );

            // Find this node in layout
            let cursor = match layout_node_index.iter().position(|&x| x == index) {
                Some(c) => c,
                None => continue,
            };

            // Skip blacklisted elements
            if BLACKLISTED_ELEMENTS.contains(&node_name.as_str()) {
                continue;
            }

            // Get element bounds
            let bound = &bounds[cursor];
            let x = bound[0] / device_pixel_ratio;
            let y = bound[1] / device_pixel_ratio;
            let width = bound[2] / device_pixel_ratio;
            let height = bound[3] / device_pixel_ratio;

            let elem_left_bound = x;
            let elem_top_bound = y;
            let elem_right_bound = x + width;
            let elem_lower_bound = y + height;

            // Check if element is partially in viewport
            let partially_is_in_viewport = elem_left_bound < win_right_bound
                && elem_right_bound >= win_left_bound
                && elem_top_bound < win_lower_bound
                && elem_lower_bound >= win_upper_bound;

            if !partially_is_in_viewport {
                continue;
            }

            // Extract element attributes
            let element_attributes = Self::find_attributes(
                &attributes[index],
                &["type", "placeholder", "aria-label", "title", "alt"],
                strings,
            );

            let is_ancestor_of_anchor = anchor_ancestry
                .get(&index.to_string())
                .map(|(b, _)| *b)
                .unwrap_or(false);
            let is_ancestor_of_button = button_ancestry
                .get(&index.to_string())
                .map(|(b, _)| *b)
                .unwrap_or(false);
            let ancestor_exception = is_ancestor_of_anchor || is_ancestor_of_button;
            let ancestor_node_key = if !ancestor_exception {
                None
            } else if is_ancestor_of_anchor {
                anchor_id.map(|id| id.to_string())
            } else {
                button_id.map(|id| id.to_string())
            };

            // Handle text nodes
            if node_name == "#text" && ancestor_exception {
                if let Some(key) = ancestor_node_key {
                    let text_index = node_value[index];
                    if text_index >= 0 {
                        let text = &strings[text_index as usize];
                        if text != "|" && text != "\"" {
                            child_nodes
                                .entry(key)
                                .or_default()
                                .push(ChildNode::Text(text.clone()));
                        }
                    }
                }
            } else {
                // Handle element nodes
                let mut node_name_mut = node_name.clone();
                let mut meta_data: Vec<String> = Vec::new();

                // Normalize button elements
                if (node_name == "input"
                    && element_attributes.get("type").map(|s| s.as_str()) == Some("submit"))
                    || node_name == "button"
                {
                    node_name_mut = "button".to_string();
                }

                // Process attributes
                for (key, value) in element_attributes.iter() {
                    if key == "type" && node_name_mut == "button" {
                        continue; // Don't add redundant (button) label
                    }
                    if ancestor_exception {
                        if let Some(ancestor_key) = &ancestor_node_key {
                            child_nodes.entry(ancestor_key.clone()).or_default().push(
                                ChildNode::Attribute {
                                    key: key.clone(),
                                    value: value.clone(),
                                },
                            );
                        }
                    } else {
                        meta_data.push(value.clone());
                    }
                }

                // Get element node value
                let mut element_node_value = None;
                if node_value[index] >= 0 {
                    let val = &strings[node_value[index] as usize];
                    if val != "|" {
                        element_node_value = Some(val.clone());
                    }
                } else if node_name == "input" && input_value_index.contains(&index) {
                    if let Some(node_input_text_index) =
                        input_value_index.iter().position(|&x| x == index)
                    {
                        let text_index = input_value_values[node_input_text_index];
                        if text_index >= 0 {
                            element_node_value = Some(strings[text_index as usize].clone());
                        }
                    }
                }

                // Remove redundant elements (children of anchors/buttons)
                if ancestor_exception && node_name != "a" && node_name != "button" {
                    continue;
                }

                elements_in_view_port.push(ElementInViewPort {
                    node_index: index.to_string(),
                    backend_node_id: backend_node_id[index],
                    node_name: Some(node_name_mut),
                    node_value: element_node_value,
                    node_meta: meta_data,
                    is_clickable: is_clickable.contains(&index),
                    origin_x: x as i32,
                    origin_y: y as i32,
                    center_x: (x + width / 2.0) as i32,
                    center_y: (y + height / 2.0) as i32,
                });
            }
        }

        // Filter and format elements
        let mut elements_of_interest = Vec::new();
        let mut id_counter = 0;

        for element in elements_in_view_port {
            let node_index = &element.node_index;
            let node_name = element.node_name.as_deref();
            let element_node_value = element.node_value.as_deref();
            let node_is_clickable = element.is_clickable;
            let mut node_meta_data = element.node_meta.clone();

            let mut inner_text = element_node_value
                .map(|s| format!("{} ", s))
                .unwrap_or_default();

            // Merge child node content
            if let Some(children) = child_nodes.get(node_index) {
                for child in children {
                    match child {
                        ChildNode::Attribute { key, value } => {
                            node_meta_data.push(format!("{}={:?}", key, value));
                        }
                        ChildNode::Text(text) => {
                            inner_text.push_str(text);
                            inner_text.push(' ');
                        }
                    }
                }
            }

            let meta = if node_meta_data.is_empty() {
                String::new()
            } else {
                format!(" {}", node_meta_data.join(" "))
            };

            let inner_text = inner_text.trim();

            let converted_node_name = Self::convert_name(node_name, node_is_clickable);

            // Skip elements with no content
            if (converted_node_name != "button" || meta.is_empty())
                && !matches!(converted_node_name, "link" | "input" | "img" | "textarea")
                && inner_text.is_empty()
            {
                continue;
            }

            self.page_element_buffer.insert(id_counter, element);

            if !inner_text.is_empty() {
                elements_of_interest.push(format!(
                    "<{} id={}{}>{}</{}>",
                    converted_node_name, id_counter, meta, inner_text, converted_node_name
                ));
            } else {
                elements_of_interest.push(format!(
                    "<{} id={}{}/>",
                    converted_node_name, id_counter, meta
                ));
            }
            id_counter += 1;
        }

        let elapsed = start.elapsed().as_secs_f64();
        println!("Parsing time: {:.2} sec ", elapsed);
        Ok(elements_of_interest)
    }

    /// Extract DOM snapshot using JavaScript evaluation.
    ///
    /// This is a fallback for CDP which may not be available in Rust playwright.
    async fn extract_dom_snapshot(&self) -> Result<DomSnapshot, Error> {
        // This is a simplified JavaScript-based DOM extraction
        // In production, this should use CDP if available for better performance
        self.browser_state
            .eval(EXTRACT_DOM_JS)
            .await
            .map_err(|e| Error::tool_error(format!("Failed to extract DOM snapshot: {}", e)))
    }

    /// Convert node name to simplified tag name.
    fn convert_name(node_name: Option<&str>, has_click_handler: bool) -> &'static str {
        match node_name {
            Some("a") => "link",
            Some("input") => "input",
            Some("img") => "img",
            Some("button") => "button",
            _ if has_click_handler => "button",
            _ => "text",
        }
    }

    /// Find specific attributes in an element's attribute list.
    fn find_attributes(
        attrs: &[i32],
        keys: &[&str],
        strings: &[String],
    ) -> HashMap<String, String> {
        let mut values = HashMap::new();
        let mut remaining_keys: HashSet<&str> = keys.iter().copied().collect();

        let mut iter = attrs.iter();
        while let (Some(&key_index), Some(&value_index)) = (iter.next(), iter.next()) {
            if value_index < 0 {
                continue;
            }
            let key = &strings[key_index as usize];
            let value = &strings[value_index as usize];

            if remaining_keys.contains(key.as_str()) {
                values.insert(key.clone(), value.clone());
                remaining_keys.remove(key.as_str());

                if remaining_keys.is_empty() {
                    break;
                }
            }
        }

        values
    }

    /// Add a node to the ancestry hash tree.
    #[allow(clippy::too_many_arguments)] // DOM traversal requires multiple parallel arrays from parser
    fn add_to_hash_tree(
        hash_tree: &mut HashMap<String, (bool, Option<usize>)>,
        tag: &str,
        node_id: usize,
        node_name: &str,
        parent_id: usize,
        node_names: &[i32],
        parent_indices: &[i32],
        strings: &[String],
    ) -> (bool, Option<usize>) {
        let parent_id_str = parent_id.to_string();

        if !hash_tree.contains_key(&parent_id_str) {
            if parent_id < node_names.len() {
                let parent_name = strings[node_names[parent_id] as usize].to_lowercase();
                let grand_parent_id = parent_indices[parent_id] as usize;

                Self::add_to_hash_tree(
                    hash_tree,
                    tag,
                    parent_id,
                    &parent_name,
                    grand_parent_id,
                    node_names,
                    parent_indices,
                    strings,
                );
            } else {
                // Root node
                hash_tree.insert(parent_id_str.clone(), (false, None));
            }
        }

        let (is_parent_desc_anchor, anchor_id) = hash_tree[&parent_id_str];

        let value = if node_name == tag {
            (true, Some(node_id))
        } else if is_parent_desc_anchor {
            (true, anchor_id)
        } else {
            (false, None)
        };

        hash_tree.insert(node_id.to_string(), value);
        value
    }
}

#[derive(Debug, Clone)]
enum ChildNode {
    Attribute { key: String, value: String },
    Text(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==================== convert_name tests ====================

    #[test]
    fn test_convert_name_anchor() {
        assert_eq!(Crawler::convert_name(Some("a"), false), "link");
        assert_eq!(Crawler::convert_name(Some("a"), true), "link");
    }

    #[test]
    fn test_convert_name_input() {
        assert_eq!(Crawler::convert_name(Some("input"), false), "input");
        assert_eq!(Crawler::convert_name(Some("input"), true), "input");
    }

    #[test]
    fn test_convert_name_img() {
        assert_eq!(Crawler::convert_name(Some("img"), false), "img");
        assert_eq!(Crawler::convert_name(Some("img"), true), "img");
    }

    #[test]
    fn test_convert_name_button() {
        assert_eq!(Crawler::convert_name(Some("button"), false), "button");
        assert_eq!(Crawler::convert_name(Some("button"), true), "button");
    }

    #[test]
    fn test_convert_name_clickable_element() {
        // Non-button elements with click handler become buttons
        assert_eq!(Crawler::convert_name(Some("div"), true), "button");
        assert_eq!(Crawler::convert_name(Some("span"), true), "button");
    }

    #[test]
    fn test_convert_name_non_clickable_element() {
        // Non-clickable elements become text
        assert_eq!(Crawler::convert_name(Some("div"), false), "text");
        assert_eq!(Crawler::convert_name(Some("span"), false), "text");
        assert_eq!(Crawler::convert_name(Some("p"), false), "text");
    }

    #[test]
    fn test_convert_name_none() {
        // None with no click handler
        assert_eq!(Crawler::convert_name(None, false), "text");
        // None with click handler
        assert_eq!(Crawler::convert_name(None, true), "button");
    }

    // ==================== find_attributes tests ====================

    #[test]
    fn test_find_attributes_empty_attrs() {
        let strings = vec!["type".to_string(), "text".to_string()];
        let attrs: Vec<i32> = vec![];
        let keys = ["type", "placeholder"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_attributes_single_match() {
        let strings = vec![
            "type".to_string(),      // 0
            "text".to_string(),      // 1
            "placeholder".to_string(), // 2
            "Enter name".to_string(), // 3
        ];
        let attrs: Vec<i32> = vec![0, 1]; // type=text
        let keys = ["type", "placeholder"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("type"), Some(&"text".to_string()));
    }

    #[test]
    fn test_find_attributes_multiple_matches() {
        let strings = vec![
            "type".to_string(),         // 0
            "text".to_string(),         // 1
            "placeholder".to_string(),  // 2
            "Enter name".to_string(),   // 3
            "aria-label".to_string(),   // 4
            "Name field".to_string(),   // 5
        ];
        let attrs: Vec<i32> = vec![0, 1, 2, 3, 4, 5]; // type=text, placeholder=Enter name, aria-label=Name field
        let keys = ["type", "placeholder", "aria-label"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        assert_eq!(result.len(), 3);
        assert_eq!(result.get("type"), Some(&"text".to_string()));
        assert_eq!(result.get("placeholder"), Some(&"Enter name".to_string()));
        assert_eq!(result.get("aria-label"), Some(&"Name field".to_string()));
    }

    #[test]
    fn test_find_attributes_no_matching_keys() {
        let strings = vec![
            "class".to_string(),    // 0
            "btn-primary".to_string(), // 1
            "id".to_string(),       // 2
            "submit".to_string(),   // 3
        ];
        let attrs: Vec<i32> = vec![0, 1, 2, 3]; // class=btn-primary, id=submit
        let keys = ["type", "placeholder"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        assert!(result.is_empty());
    }

    #[test]
    fn test_find_attributes_negative_value_index() {
        let strings = vec![
            "type".to_string(),     // 0
            "text".to_string(),     // 1
        ];
        // Value index is -1 (no value)
        let attrs: Vec<i32> = vec![0, -1];
        let keys = ["type"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        assert!(result.is_empty()); // Should skip entries with negative value index
    }

    #[test]
    fn test_find_attributes_odd_length_attrs() {
        let strings = vec![
            "type".to_string(),     // 0
            "text".to_string(),     // 1
        ];
        // Odd number of attributes (incomplete pair)
        let attrs: Vec<i32> = vec![0, 1, 0];
        let keys = ["type"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        // Should still find the first complete pair
        assert_eq!(result.len(), 1);
        assert_eq!(result.get("type"), Some(&"text".to_string()));
    }

    #[test]
    fn test_find_attributes_early_exit() {
        let strings = vec![
            "type".to_string(),         // 0
            "text".to_string(),         // 1
            "placeholder".to_string(),  // 2
            "Enter name".to_string(),   // 3
            "class".to_string(),        // 4
            "input-field".to_string(),  // 5
        ];
        // All requested keys appear early
        let attrs: Vec<i32> = vec![0, 1, 2, 3, 4, 5];
        let keys = ["type", "placeholder"];

        let result = Crawler::find_attributes(&attrs, &keys, &strings);
        // Should find both keys and exit early (not process class)
        assert_eq!(result.len(), 2);
    }

    // ==================== BLACKLISTED_ELEMENTS tests ====================

    #[test]
    fn test_blacklisted_elements_contains_structural() {
        assert!(BLACKLISTED_ELEMENTS.contains(&"html"));
        assert!(BLACKLISTED_ELEMENTS.contains(&"head"));
        assert!(BLACKLISTED_ELEMENTS.contains(&"body"));
    }

    #[test]
    fn test_blacklisted_elements_contains_metadata() {
        assert!(BLACKLISTED_ELEMENTS.contains(&"title"));
        assert!(BLACKLISTED_ELEMENTS.contains(&"meta"));
    }

    #[test]
    fn test_blacklisted_elements_contains_non_visual() {
        assert!(BLACKLISTED_ELEMENTS.contains(&"script"));
        assert!(BLACKLISTED_ELEMENTS.contains(&"style"));
    }

    #[test]
    fn test_blacklisted_elements_contains_svg() {
        assert!(BLACKLISTED_ELEMENTS.contains(&"svg"));
        assert!(BLACKLISTED_ELEMENTS.contains(&"path"));
    }

    #[test]
    fn test_blacklisted_elements_does_not_contain_interactive() {
        assert!(!BLACKLISTED_ELEMENTS.contains(&"a"));
        assert!(!BLACKLISTED_ELEMENTS.contains(&"button"));
        assert!(!BLACKLISTED_ELEMENTS.contains(&"input"));
    }

    // ==================== ElementInViewPort tests ====================

    #[test]
    fn test_element_in_viewport_serialize() {
        let element = ElementInViewPort {
            node_index: "42".to_string(),
            backend_node_id: 123,
            node_name: Some("button".to_string()),
            node_value: Some("Click me".to_string()),
            node_meta: vec!["type=submit".to_string()],
            is_clickable: true,
            origin_x: 100,
            origin_y: 200,
            center_x: 150,
            center_y: 225,
        };

        let json = serde_json::to_string(&element).expect("serialize");
        assert!(json.contains("\"node_index\":\"42\""));
        assert!(json.contains("\"backend_node_id\":123"));
        assert!(json.contains("\"is_clickable\":true"));
    }

    #[test]
    fn test_element_in_viewport_deserialize() {
        let json = r#"{
            "node_index": "10",
            "backend_node_id": 456,
            "node_name": "a",
            "node_value": "Home",
            "node_meta": [],
            "is_clickable": true,
            "origin_x": 0,
            "origin_y": 0,
            "center_x": 50,
            "center_y": 25
        }"#;

        let element: ElementInViewPort = serde_json::from_str(json).expect("deserialize");
        assert_eq!(element.node_index, "10");
        assert_eq!(element.backend_node_id, 456);
        assert_eq!(element.node_name, Some("a".to_string()));
        assert!(element.is_clickable);
    }

    #[test]
    fn test_element_in_viewport_clone() {
        let element = ElementInViewPort {
            node_index: "1".to_string(),
            backend_node_id: 1,
            node_name: None,
            node_value: None,
            node_meta: vec![],
            is_clickable: false,
            origin_x: 0,
            origin_y: 0,
            center_x: 0,
            center_y: 0,
        };

        let cloned = element.clone();
        assert_eq!(element.node_index, cloned.node_index);
        assert_eq!(element.backend_node_id, cloned.backend_node_id);
    }

    #[test]
    fn test_element_in_viewport_debug() {
        let element = ElementInViewPort {
            node_index: "1".to_string(),
            backend_node_id: 1,
            node_name: Some("div".to_string()),
            node_value: None,
            node_meta: vec![],
            is_clickable: false,
            origin_x: 0,
            origin_y: 0,
            center_x: 0,
            center_y: 0,
        };

        let debug = format!("{:?}", element);
        assert!(debug.contains("ElementInViewPort"));
        assert!(debug.contains("node_index"));
    }

    // ==================== ChildNode tests ====================

    #[test]
    fn test_child_node_text_clone() {
        let node = ChildNode::Text("Hello".to_string());
        let cloned = node.clone();
        if let ChildNode::Text(text) = cloned {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected Text variant");
        }
    }

    #[test]
    fn test_child_node_attribute_clone() {
        let node = ChildNode::Attribute {
            key: "href".to_string(),
            value: "https://example.com".to_string(),
        };
        let cloned = node.clone();
        if let ChildNode::Attribute { key, value } = cloned {
            assert_eq!(key, "href");
            assert_eq!(value, "https://example.com");
        } else {
            panic!("Expected Attribute variant");
        }
    }

    #[test]
    fn test_child_node_debug() {
        let text_node = ChildNode::Text("test".to_string());
        let debug = format!("{:?}", text_node);
        assert!(debug.contains("Text"));
        assert!(debug.contains("test"));

        let attr_node = ChildNode::Attribute {
            key: "id".to_string(),
            value: "main".to_string(),
        };
        let debug = format!("{:?}", attr_node);
        assert!(debug.contains("Attribute"));
        assert!(debug.contains("id"));
    }

    // ==================== DOM snapshot structures tests ====================

    #[test]
    fn test_clickable_index_deserialize() {
        let json = r#"{"index": [1, 5, 10]}"#;
        let clickable: ClickableIndex = serde_json::from_str(json).expect("deserialize");
        assert_eq!(clickable.index, vec![1, 5, 10]);
    }

    #[test]
    fn test_input_value_deserialize() {
        let json = r#"{"index": [0, 2], "value": [3, 4]}"#;
        let input: InputValue = serde_json::from_str(json).expect("deserialize");
        assert_eq!(input.index, vec![0, 2]);
        assert_eq!(input.value, vec![3, 4]);
    }

    #[test]
    fn test_layout_deserialize() {
        let json = r#"{"nodeIndex": [0, 1], "bounds": [[0, 0, 100, 50], [100, 0, 200, 50]]}"#;
        let layout: Layout = serde_json::from_str(json).expect("deserialize");
        assert_eq!(layout.node_index, vec![0, 1]);
        assert_eq!(layout.bounds.len(), 2);
        assert_eq!(layout.bounds[0], vec![0.0, 0.0, 100.0, 50.0]);
    }
}
