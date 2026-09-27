// Images the library ships (Pip's poses). The desktop app gets the same declaration from Vite.
declare module "*.png" {
  const src: string;
  export default src;
}
