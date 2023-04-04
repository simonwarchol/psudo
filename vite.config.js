import {defineConfig} from 'vite'
import react from '@vitejs/plugin-react'
import {glslify} from 'vite-plugin-glslify'
import svgr from 'vite-plugin-svgr'
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

// https://vitejs.dev/config/
export default defineConfig({
    plugins: [react(), glslify(), svgr(), wasm(),
        topLevelAwait()],
    define: {
        'process.env': {}
    }
})
