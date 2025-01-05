const { fontFamily } = require("tailwindcss/defaultTheme");

/** @type {import('tailwindcss').Config} */
module.exports = {
  corePlugins: {
    preflight: false,
    container: false,
  },
  darkMode: ["class", '[data-theme="dark"]'],
  content: ["./src/**/*.{jsx,tsx,html}", "./docs/**/*.{md,mdx}"],
  theme: {
    extend: {
      fontFamily: {
        sans: ['"Inter"', ...fontFamily.sans],
        jakarta: ['"Plus Jakarta Sans"', ...fontFamily.sans],
        mono: ['"Fira Code"', ...fontFamily.mono],
      },
      borderRadius: {
        sm: "4px",
      },
      screens: {
        sm: "0px",
        lg: "997px",
      },
      colors: {},
    },
  },
  plugins: [],
};

// module.exports = {
//   content: ["./src/**/*.{js,jsx,ts,tsx,md,mdx}", "./docs/**/*.{md,mdx}"],
//   theme: {
//     extend: {},
//   },
//   plugins: [
//     async function myPlugin(context, options) {
//       return {
//         name: "docusaurus-tailwindcss",
//         configurePostCss(postcssOptions) {
//           // Appends TailwindCSS and AutoPrefixer.
//           postcssOptions.plugins.push(require("tailwindcss"));
//           postcssOptions.plugins.push(require("autoprefixer"));
//           return postcssOptions;
//         },
//       };
//     },
//   ],
// };