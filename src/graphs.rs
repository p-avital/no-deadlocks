use crate::{Map, Set};

#[derive(Debug)]
pub struct Graph<NodeType: std::cmp::PartialEq + std::cmp::Eq + std::hash::Hash> {
    nodes: Map<NodeType, Set<NodeType>>
}

impl<NodeType: std::cmp::PartialEq + std::cmp::Eq + std::hash::Hash> Graph<NodeType> {
    pub fn new() -> Self {
        Graph {nodes: Map::new()}
    }
    #[allow(dead_code)]
    pub fn add_node(&mut self, node: NodeType) {
        if !self.nodes.contains_key(&node) {
            self.nodes.insert(node, Set::new());
        }
    }
    #[allow(dead_code)]
    pub fn add_edge(&mut self, from: &NodeType, to: NodeType) {
        self.nodes.get_mut(from).unwrap().insert(to, ());
    }

    pub fn add_edge_and_nodes(&mut self, from: NodeType, to: NodeType) where NodeType: Clone {
        if !self.nodes.contains_key(&to) {
            self.nodes.insert(to.clone(), Set::new());
        }
        if !self.nodes.contains_key(&from) {
            let mut set = Set::new();
            set.insert(to, ());
            self.nodes.insert(from, set);
        } else {
            self.nodes.get_mut(&from).unwrap().insert(to, ());
        }
    }

    pub fn find_loop(&self) -> Option<Vec<&NodeType>> {
        let mut history = Vec::new();
        for node in self.nodes.keys() {
            history.push(node);
            if self.loop_backtracker(&mut history) {
                return Some(history);
            }
            history.pop();
        }
        None
    }

    fn loop_backtracker<'l>(&'l self, history: &mut Vec<&'l NodeType>) -> bool {
        let current_node = *history.last().unwrap();
        for node in self.nodes.get(current_node).unwrap().keys() {
            if let Some(start) = history.iter().position(|prev| *prev == node) {
                history.rotate_left(start);
                for _ in 0..start {
                    history.pop();
                }
                return true;
            }
            history.push(node);
            if self.loop_backtracker(history) {
                return true;
            }
            history.pop();
        }
        false
    }
}

#[test]
fn graph_loop_detection() {
    let mut graph = Graph::new();
    graph.add_edge_and_nodes(0, 1);
    graph.add_edge_and_nodes(0, 2);
    graph.add_edge_and_nodes(0, 3);
    graph.add_edge_and_nodes(1, 2);
    graph.add_edge_and_nodes(4, 0);
    if let Some(r#loop) = graph.find_loop() {
        panic!("There shouldn't be any loop yet ! {:?}", r#loop)
    }
    graph.add_edge_and_nodes(2, 4);
    if let Some(r#loop) = graph.find_loop() {
        println!("Found loop! {:?}", r#loop)
    } else{
        panic!("There should be a loop here !")
    }
}