export default {
    name: "Conway's Game of Life",
    description:
        "This example shows how to make Conway's game of life. First, use compute shader to calculate how cells grow or die. Then use render pipeline to draw cells by using instance mesh.",
    filename: "src/meta.ts",
    sources: [
        { path: "main.ts" },
        { path: "shaders/compute.wgsl" },
        { path: "shaders/vert.wgsl" },
        { path: "shaders/frag.wgsl" },
    ],
};
