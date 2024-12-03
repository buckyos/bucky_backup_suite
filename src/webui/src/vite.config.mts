import { defineConfig } from "vite";
import { resolve } from "path";
import dts from 'vite-plugin-dts'
import { viteStaticCopy } from 'vite-plugin-static-copy'

export default defineConfig({
  optimizeDeps: {
    exclude: ['@mapbox/node-pre-gyp', 'mock-aws-s3', 'aws-sdk', 'nock']
  },
  build: {
    minify: 'terser',
    sourcemap: true,
    rollupOptions: {
      external: ['@mapbox/node-pre-gyp','mock-aws-s3', 'aws-sdk', 'nock'],
      input: {
        index: resolve(__dirname,"index.html"),
      }
    }
  },
  resolve: {
    alias: {
      "@": __dirname,
      '/shoelace': resolve(__dirname, 'node_modules/@shoelace-style/shoelace/')
    }
  },
  plugins: [
    dts(),
    viteStaticCopy({
      targets: [
        {
          src: 'node_modules/@shoelace-style/shoelace/dist/assets/*',
          dest: 'assets'
        }
      ]
    })
  ]
});