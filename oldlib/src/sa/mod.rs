extern crate rand;

use std::f32;

use rand::Rng;

fn f(x: Vec<f32>) -> f32 {
    // (x+5)^2 + 2x + (y-10)^2 + 2y + 1
    (x[0] + 5.0).powi(2) + 2.0 * x[0] + (x[1] - 10.0).powi(2) + 2.0 * x[1] + 1.0
}

fn random_neighbor(x: Vec<f32>, step: f32) -> ( Vec<f32>) {
    let mut rng = rand::thread_rng();
    let dx = rng.gen_range(-step..=step);
    let dy = rng.gen_range(-step..=step);
    vec![x[0] + dx, x[1] + dy]
}

fn acceptance_probability(current_energy: f32, new_energy: f32, temperature: f32) -> f32 {
    if new_energy < current_energy {
        1.0
    } else {
        ((current_energy - new_energy) / temperature).exp()
    }
}

fn simulated_annealing(
    initial_temperature: f32,
    cooling_rate: f32,
    max_iterations: usize,
    initial_solution: Vec<f32>,
) -> Vec<f32> {
    let mut current_solution = initial_solution;
    let mut current_energy = f(current_solution.clone());
    let mut temperature = initial_temperature;

    for i in 0..max_iterations {
        let step = temperature / initial_temperature;
        let new_solution = random_neighbor(current_solution.clone(), step);
        let new_energy = f(new_solution.clone());

        let ap = acceptance_probability(current_energy, new_energy, temperature);
        let mut rng = rand::thread_rng();
        if ap > rng.gen_range(0.0..=1.0) {
            current_solution = new_solution.clone();
            current_energy = new_energy;
        }

        temperature *= cooling_rate;
    }
    current_solution.clone()
}

pub fn anneal() -> Vec<f32> {
    let initial_temperature = 1000.0;
    let cooling_rate = 0.995;
    let max_iterations = 10000;
    let initial_solution = vec![0.0, 0.0];

    let solution = simulated_annealing(
        initial_temperature,
        cooling_rate,
        max_iterations,
        initial_solution,
    );
    println!("Found local minimum at: {:?}", solution);
    solution
}