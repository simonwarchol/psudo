// vite.config.js
import { defineConfig } from "file:///Users/swarchol/Research/psudo/node_modules/vite/dist/node/index.js";
import react from "file:///Users/swarchol/Research/psudo/node_modules/@vitejs/plugin-react/dist/index.mjs";
import { glslify } from "file:///Users/swarchol/Research/psudo/node_modules/vite-plugin-glslify/dist/index.js";
import svgr from "file:///Users/swarchol/Research/psudo/node_modules/vite-plugin-svgr/dist/index.mjs";
import wasm from "file:///Users/swarchol/Research/psudo/node_modules/vite-plugin-wasm/exports/import.mjs";
import topLevelAwait from "file:///Users/swarchol/Research/psudo/node_modules/vite-plugin-top-level-await/exports/import.mjs";
var vite_config_default = defineConfig({
  plugins: [
    react(),
    glslify(),
    svgr(),
    wasm(),
    topLevelAwait()
  ],
  define: {
    "process.env": {}
  }
});
export {
  vite_config_default as default
};
//# sourceMappingURL=data:application/json;base64,ewogICJ2ZXJzaW9uIjogMywKICAic291cmNlcyI6IFsidml0ZS5jb25maWcuanMiXSwKICAic291cmNlc0NvbnRlbnQiOiBbImNvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9kaXJuYW1lID0gXCIvVXNlcnMvc3dhcmNob2wvUmVzZWFyY2gvcHN1ZG9cIjtjb25zdCBfX3ZpdGVfaW5qZWN0ZWRfb3JpZ2luYWxfZmlsZW5hbWUgPSBcIi9Vc2Vycy9zd2FyY2hvbC9SZXNlYXJjaC9wc3Vkby92aXRlLmNvbmZpZy5qc1wiO2NvbnN0IF9fdml0ZV9pbmplY3RlZF9vcmlnaW5hbF9pbXBvcnRfbWV0YV91cmwgPSBcImZpbGU6Ly8vVXNlcnMvc3dhcmNob2wvUmVzZWFyY2gvcHN1ZG8vdml0ZS5jb25maWcuanNcIjtpbXBvcnQge2RlZmluZUNvbmZpZ30gZnJvbSAndml0ZSdcbmltcG9ydCByZWFjdCBmcm9tICdAdml0ZWpzL3BsdWdpbi1yZWFjdCdcbmltcG9ydCB7Z2xzbGlmeX0gZnJvbSAndml0ZS1wbHVnaW4tZ2xzbGlmeSdcbmltcG9ydCBzdmdyIGZyb20gJ3ZpdGUtcGx1Z2luLXN2Z3InXG5pbXBvcnQgd2FzbSBmcm9tIFwidml0ZS1wbHVnaW4td2FzbVwiO1xuaW1wb3J0IHRvcExldmVsQXdhaXQgZnJvbSBcInZpdGUtcGx1Z2luLXRvcC1sZXZlbC1hd2FpdFwiO1xuXG4vLyBodHRwczovL3ZpdGVqcy5kZXYvY29uZmlnL1xuZXhwb3J0IGRlZmF1bHQgZGVmaW5lQ29uZmlnKHtcbiAgICBwbHVnaW5zOiBbcmVhY3QoKSwgZ2xzbGlmeSgpLCBzdmdyKCksIHdhc20oKSxcbiAgICAgICAgdG9wTGV2ZWxBd2FpdCgpXSxcbiAgICBkZWZpbmU6IHtcbiAgICAgICAgJ3Byb2Nlc3MuZW52Jzoge31cbiAgICB9XG59KVxuIl0sCiAgIm1hcHBpbmdzIjogIjtBQUE0USxTQUFRLG9CQUFtQjtBQUN2UyxPQUFPLFdBQVc7QUFDbEIsU0FBUSxlQUFjO0FBQ3RCLE9BQU8sVUFBVTtBQUNqQixPQUFPLFVBQVU7QUFDakIsT0FBTyxtQkFBbUI7QUFHMUIsSUFBTyxzQkFBUSxhQUFhO0FBQUEsRUFDeEIsU0FBUztBQUFBLElBQUMsTUFBTTtBQUFBLElBQUcsUUFBUTtBQUFBLElBQUcsS0FBSztBQUFBLElBQUcsS0FBSztBQUFBLElBQ3ZDLGNBQWM7QUFBQSxFQUFDO0FBQUEsRUFDbkIsUUFBUTtBQUFBLElBQ0osZUFBZSxDQUFDO0FBQUEsRUFDcEI7QUFDSixDQUFDOyIsCiAgIm5hbWVzIjogW10KfQo=
