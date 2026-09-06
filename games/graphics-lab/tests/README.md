# Graphics Lab tests

Graphics Lab is primarily verified by the dedicated browser workflow in `.github/workflows/graphics-lab.yml`.
That workflow exports the project, runs it through the shared WebAssembly/WebGPU host, captures the rendered scene in Chromium, and uploads the screenshot as an artifact.

Keep this project texture-free. A future regression test may inspect the exported manifest and fail if a texture or sprite asset is introduced.
