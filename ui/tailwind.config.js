/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        canvas: "#0a0a0a",
        card: "#111111",
        ink: "#fafafa",
        muted: "#a1a1aa",
      },
    },
  },
  plugins: [],
};
