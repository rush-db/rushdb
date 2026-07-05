function tailwindPlugin(context, options) {
  return {
    name: 'tailwind-plugin',
    configurePostCss(postcssOptions) {
      // Tailwind v4: a single PostCSS plugin handles imports, vendor prefixing
      // and utilities. Docusaurus's default plugins stay in the chain.
      postcssOptions.plugins.unshift(require('@tailwindcss/postcss'));
      return postcssOptions;
    },
  };
}

module.exports = tailwindPlugin;
