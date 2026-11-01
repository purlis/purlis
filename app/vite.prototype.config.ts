import { defineConfig, loadConfigFromFile } from "vite";
import { handedThemes } from "./.storybook/handed.plugin.ts";
const stub={name:"proto-stub",resolveId:(a:string)=>a==="virtual:purlis-third-party"?"\0tp":undefined,load:(i:string)=>i==="\0tp"?"export default []":undefined};
export default defineConfig(async (env) => {
  const base:any = (await loadConfigFromFile(env,"vite.config.ts"))!.config;
  return {...base, server:{port:1477,strictPort:true,hmr:{overlay:false}}, plugins:[stub,...(base.plugins as any[]).flat().filter((p:any)=>p?.name!=="purlis-third-party-notices"),handedThemes()]};
});
