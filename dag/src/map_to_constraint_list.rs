use super::{Constraint, Edge, Node, SimplificationFlags, Tree, DAG};
use constraint_list::{NodeData, ConstraintList, DAGEncoding, EncodingEdge, EncodingNode, SignalInfo, Simplifier, ConstraintCounter};
use program_structure::utils::constants::UsefulConstants;
use std::{array::IntoIter, collections::{HashMap, HashSet, LinkedList}};
#[derive(Default)]
struct CHolder {
    linear: LinkedList<Constraint>,
    equalities: LinkedList<Constraint>,
    constant_equalities: LinkedList<Constraint>,
}



fn map_tree(
    tree: &Tree,
    witness: &mut Vec<usize>,
    c_holder: &mut CHolder,
    forbidden: &mut HashSet<usize>,
    init_constraint_counter: &mut ConstraintCounter,
    nodes_data: &mut Vec<NodeData>,
    template_to_nodes: &mut HashMap<String, Vec<usize>>,
) -> ConstraintCounter{
    let mut counter = ConstraintCounter{
        num_constant_eq: 0,
        num_signal_eq: 0,
        num_linear_eq: 0,
        num_no_linear_eq: 0
    };
    let copy_init_counter = init_constraint_counter.clone();

    for signal in &tree.signals {
        Vec::push(witness, *signal);
        if tree.dag.nodes[tree.node_id].is_custom_gate {
            forbidden.insert(*signal);
        }
    }

    let initial_signal = if tree.signals.len() > 0{
        tree.signals[0]
    } else{
        0
    };

    for constraint in &tree.constraints {
        if Constraint::is_constant_equality(constraint) {
            LinkedList::push_back(&mut c_holder.constant_equalities, constraint.clone());
            counter.num_constant_eq += 1;
        } else if Constraint::is_equality(constraint, &tree.field) {
            LinkedList::push_back(&mut c_holder.equalities, constraint.clone());
            counter.num_signal_eq += 1;
        } else if Constraint::is_linear(constraint) {
            LinkedList::push_back(&mut c_holder.linear, constraint.clone());
            counter.num_linear_eq += 1;
        } else {
            counter.num_no_linear_eq += 1;
        }
    }
    init_constraint_counter.num_constant_eq += counter.num_constant_eq; 
    init_constraint_counter.num_signal_eq += counter.num_signal_eq; 
    init_constraint_counter.num_linear_eq += counter.num_linear_eq; 
    init_constraint_counter.num_no_linear_eq += counter.num_no_linear_eq; 

    for edge in Tree::get_edges(tree) {
        let subtree = Tree::go_to_subtree(tree, edge);
        let aux_counter= map_tree(
            &subtree, 
            witness, 
            c_holder, 
            forbidden,
            init_constraint_counter,
            nodes_data,
            template_to_nodes
        );
        counter.num_constant_eq += aux_counter.num_constant_eq;
        counter.num_signal_eq += aux_counter.num_signal_eq;
        counter.num_linear_eq += aux_counter.num_linear_eq;
        counter.num_no_linear_eq += aux_counter.num_no_linear_eq;

    }

    let node_data = NodeData{
        template_instance: tree.template_name.clone(),
        number_inputs: tree.number_inputs,
        number_outputs: tree.number_outputs,
        initial_signal,
        num_constraint_counter: counter.clone(),
        init_constraint_counter: copy_init_counter,
    };
    if template_to_nodes.contains_key(&node_data.template_instance){
        let info = template_to_nodes.get_mut(&node_data.template_instance).unwrap();
        info.push(nodes_data.len());
    } else{
        template_to_nodes.insert(node_data.template_instance.clone(), vec![nodes_data.len()]);
    }
    nodes_data.push(node_data);

    counter
}

fn produce_encoding(
    no_constraints: usize,
    init: usize,
    dag_nodes: Vec<Node>,
    dag_edges: Vec<Vec<Edge>>,
) -> DAGEncoding {
    let mut adjacency = Vec::new();
    let mut nodes = Vec::new();
    let mut id = 0;
    for node in dag_nodes {
        let encoded = map_node_to_encoding(id, node);
        Vec::push(&mut nodes, encoded);
        id += 1;
    }
    for edges in dag_edges {
        let mut encoded = Vec::new();
        for edge in edges {
            let new = map_edge_to_encoding(edge);
            Vec::push(&mut encoded, new);
        }
        Vec::push(&mut adjacency, encoded);
    }
    DAGEncoding { init, no_constraints, nodes, adjacency }
}

fn map_node_to_encoding(id: usize, node: Node) -> EncodingNode {
    let mut signals = Vec::new();
    let mut ordered_signals = Vec::new();
    let locals = node.locals;
    let mut non_linear = LinkedList::new();
    for c in node.constraints {
        if !Constraint::is_linear(&c) {
            LinkedList::push_back(&mut non_linear, c);
        }
    }

    for signal in node.ordered_signals {
        let signal_numbering = node.signal_correspondence.get(&signal).unwrap();
        ordered_signals.push(*signal_numbering);
    }

    for (name, id) in node.signal_correspondence {
        if HashSet::contains(&locals, &id) {
            let new_signal = SignalInfo { name, id };
            Vec::push(&mut signals, new_signal);
        }
    }
    signals.sort_by(|a, b| a.id.cmp(&b.id));

    EncodingNode {
        id,
        name: node.template_name,
        parameters: node.parameters,
        signals,
        ordered_signals,
        non_linear,
        is_custom_gate: node.is_custom_gate,
    }
}

fn map_edge_to_encoding(edge: Edge) -> EncodingEdge {
    EncodingEdge { goes_to: edge.goes_to, path: edge.label, offset: edge.in_number }
}

pub fn map(dag: DAG, flags: SimplificationFlags) -> ConstraintList {
    use std::time::SystemTime;
    // println!("Start of dag to list mapping");
    let now = SystemTime::now();
    let constants = UsefulConstants::new(&dag.prime);
    let field = constants.get_p().clone();
    let init_id = dag.main_id();
    let no_public_inputs = dag.public_inputs();
    let no_public_outputs = dag.public_outputs();
    let no_private_inputs = dag.private_inputs();
    let mut forbidden = dag.get_main().unwrap().forbidden_if_main.clone();
    let mut c_holder: CHolder = CHolder::default();
    let mut nodes_data: Vec<NodeData> = Vec::new();
    let mut template_to_nodes: HashMap<String, Vec<usize>> = HashMap::new();
    let mut signal_map = vec![0];
    let mut init_constraint_counter = ConstraintCounter{
        num_constant_eq: 0,
        num_signal_eq: 0,
        num_linear_eq: 0,
        num_no_linear_eq: 0
    };
    let counter = map_tree(
        &Tree::new(&dag), 
        &mut signal_map, 
        &mut c_holder, 
        &mut forbidden,
        &mut init_constraint_counter,
        &mut nodes_data,
        &mut template_to_nodes
    );
    let max_signal = Vec::len(&signal_map);
    let name_encoding = produce_encoding(counter.num_no_linear_eq, init_id, dag.nodes, dag.adjacency);
    let _dur = now.elapsed().unwrap().as_millis();
    // println!("End of dag to list mapping: {} ms", dur);
    Simplifier {
        field,
        no_public_inputs,
        no_public_outputs,
        no_private_inputs,
        forbidden,
        max_signal,
        dag_encoding: name_encoding,
        linear: c_holder.linear,
        equalities: c_holder.equalities,
        cons_equalities: c_holder.constant_equalities,
        no_rounds: flags.no_rounds,
        flag_s: flags.flag_s,
        parallel_flag: flags.parallel_flag,
        flag_old_heuristics: flags.flag_old_heuristics,
        port_substitution: flags.port_substitution,
        json_substitutions: flags.json_substitutions,
        nodes_data,
        template_to_nodes
    }
    .simplify_constraints()
}
