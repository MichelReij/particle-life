const path = require("path");
const HtmlWebpackPlugin = require("html-webpack-plugin");
const CopyWebpackPlugin = require("copy-webpack-plugin");
const MiniCssExtractPlugin = require("mini-css-extract-plugin");
const { exec } = require("child_process");
const fs = require("fs");

// Custom plugin to build Rust WASM before webpack compilation
class RustWasmPlugin {
    constructor() {
        this.isBuilding = false;
        this.lastBuildTime = 0;
    }

    apply(compiler) {
        compiler.hooks.beforeCompile.tapAsync(
            "RustWasmPlugin",
            (params, callback) => {
                // Avoid building if we're already building
                if (this.isBuilding) {
                    return callback();
                }

                // Check if we need to rebuild by comparing timestamps
                if (this.shouldRebuild()) {
                    this.isBuilding = true;
                    console.log("Building Rust WASM module...");
                    exec(
                        "wasm-pack build --target web --out-dir src/pkg --out-name particle_life_wasm",
                        (error, stdout, stderr) => {
                            this.isBuilding = false;
                            if (error) {
                                console.error("Rust WASM build failed:", error);
                                return callback(error);
                            }
                            if (stderr) {
                                console.log("Rust build warnings:", stderr);
                            }
                            console.log("Rust WASM build complete!");
                            this.lastBuildTime = Date.now();
                            callback();
                        }
                    );
                } else {
                    callback();
                }
            }
        );
    }

    shouldRebuild() {
        try {
            // Check if Cargo.toml exists and get its modification time
            const cargoTomlPath = path.resolve(__dirname, "Cargo.toml");
            const srcPath = path.resolve(__dirname, "src");
            const wasmPath = path.resolve(
                __dirname,
                "src/pkg/particle_life_wasm_bg.wasm"
            );

            // If WASM file doesn't exist, we need to build
            if (!fs.existsSync(wasmPath)) {
                return true;
            }

            const wasmStat = fs.statSync(wasmPath);
            const wasmTime = wasmStat.mtime.getTime();

            // Check if any Rust source files are newer than the WASM file
            const rustFiles = [
                "Cargo.toml",
                "Cargo.lock",
                ...this.getRustFiles(srcPath),
            ];

            for (const file of rustFiles) {
                const filePath = path.resolve(__dirname, file);
                if (fs.existsSync(filePath)) {
                    const fileStat = fs.statSync(filePath);
                    if (fileStat.mtime.getTime() > wasmTime) {
                        return true;
                    }
                }
            }

            return false;
        } catch (error) {
            // If we can't determine, rebuild to be safe
            return true;
        }
    }

    getRustFiles(dir) {
        const files = [];
        try {
            const items = fs.readdirSync(dir);
            for (const item of items) {
                const itemPath = path.join(dir, item);
                const stat = fs.statSync(itemPath);
                if (stat.isDirectory() && item !== "pkg") {
                    files.push(
                        ...this.getRustFiles(itemPath).map((f) =>
                            path.join(item, f)
                        )
                    );
                } else if (item.endsWith(".rs")) {
                    files.push(path.relative(__dirname, itemPath));
                }
            }
        } catch (error) {
            // Ignore errors reading directories
        }
        return files;
    }
}

module.exports = {
    entry: {
        main: "./src/main.ts",
        styles: "./src/styles/main.scss",
    },
    output: {
        filename: "[name].js",
        path: path.resolve(__dirname, "public"),
        clean: false, // Don't clean the public folder to preserve manual assets
    },
    resolve: {
        extensions: [".ts", ".js", ".wasm"],
        fallback: {
            wbg: false, // Disable wbg module resolution
        },
    },
    experiments: {
        asyncWebAssembly: true,
        topLevelAwait: true,
    },
    module: {
        rules: [
            {
                test: /\.ts$/,
                use: "ts-loader",
                exclude: /node_modules/,
            },
            {
                test: /\.wgsl$/,
                type: "asset/source",
            },
            {
                test: /\.scss$/,
                use: [MiniCssExtractPlugin.loader, "css-loader", "sass-loader"],
            },
            {
                test: /\.wasm$/,
                type: "webassembly/async",
            },
            {
                test: /particle_life_wasm\.js$/,
                type: "javascript/esm",
            },
        ],
    },
    plugins: [
        new RustWasmPlugin(),
        new HtmlWebpackPlugin({
            template: path.resolve(__dirname, "src/index.html"),
            inject: true,
            filename: "index.html",
            cache: false,
        }),
        new MiniCssExtractPlugin({
            filename: "styles.css",
        }),
        new CopyWebpackPlugin({
            patterns: [
                {
                    from: "src/shaders",
                    to: "shaders",
                },
                {
                    from: "src/pkg",
                    to: "pkg",
                    globOptions: {
                        ignore: [
                            "**/package.json",
                            "**/README.md",
                            "**/.gitignore",
                        ],
                    },
                },
                {
                    from: "src/joy.js",
                    to: "joy.js",
                },
            ],
        }),
    ],
    watchOptions: {
        ignored: ["**/src/pkg/**", "**/node_modules/**", "**/public/**"],
    },
    devServer: {
        static: {
            directory: path.join(__dirname, "public"),
        },
        port: 3000,
        hot: false, // Disable HMR to avoid WebGPU device conflicts
        liveReload: true, // Enable simple live reload instead
        watchFiles: {
            paths: ["src/**/*"],
            options: {
                ignored: [
                    "**/src/pkg/**",
                    "**/node_modules/**",
                    "**/public/**",
                ],
            },
        },
    },
    mode: "development",
};
