// Simple spatial grid for particle interactions optimization
// Divides the world into cells and only checks particles in nearby cells

use crate::particle_system::Particle;

#[derive(Debug)]
pub struct SpatialGrid {
    cell_size: f32,
    grid_width: usize,
    grid_height: usize,
    cells: Vec<Vec<usize>>, // Each cell contains particle indices
}

impl SpatialGrid {
    pub fn new(world_width: f32, world_height: f32, cell_size: f32) -> Self {
        let grid_width = (world_width / cell_size).ceil() as usize;
        let grid_height = (world_height / cell_size).ceil() as usize;
        let total_cells = grid_width * grid_height;

        Self {
            cell_size,
            grid_width,
            grid_height,
            cells: vec![Vec::new(); total_cells],
        }
    }

    pub fn clear(&mut self) {
        for cell in &mut self.cells {
            cell.clear();
        }
    }

    pub fn insert(&mut self, particle_index: usize, particle: &Particle) {
        let cell_x = (particle.position[0] / self.cell_size) as usize;
        let cell_y = (particle.position[1] / self.cell_size) as usize;

        if cell_x < self.grid_width && cell_y < self.grid_height {
            let cell_index = cell_y * self.grid_width + cell_x;
            self.cells[cell_index].push(particle_index);
        }
    }

    pub fn get_nearby_particles(&self, particle: &Particle, max_radius: f32) -> Vec<usize> {
        let mut nearby = Vec::new();

        // Calculate cell range to check
        let cells_to_check = (max_radius / self.cell_size).ceil() as i32;
        let center_x = (particle.position[0] / self.cell_size) as i32;
        let center_y = (particle.position[1] / self.cell_size) as i32;

        for dy in -cells_to_check..=cells_to_check {
            for dx in -cells_to_check..=cells_to_check {
                let x = center_x + dx;
                let y = center_y + dy;

                if x >= 0 && x < self.grid_width as i32 && y >= 0 && y < self.grid_height as i32 {
                    let cell_index = (y as usize) * self.grid_width + (x as usize);
                    for &particle_idx in &self.cells[cell_index] {
                        nearby.push(particle_idx);
                    }
                }
            }
        }

        nearby
    }
}
