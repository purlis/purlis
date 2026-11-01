// PROTOTYPE, throwaway (see prototype/onboarding-ui): a mocked core for the real window.
import { mockIPC } from "@tauri-apps/api/mocks";
export const params = new URLSearchParams(location.hash.slice(1));
const h = (name:string,title:string,installed:boolean,signed_in:boolean,installer:string,page:string)=>({name,title,installed,signed_in,installer,installer_page:page,cannot_type:null,glyph:null,own:null,monogram:title[0],says:null});
export const fresh = params.get("fresh")!=="0";
const harnesses = params.get("harness")==="none"
 ? [h("claude","Claude Code",false,false,"curl -fsSL https://claude.ai/install.sh | bash","claude.com/claude-code"),h("codex","Codex",false,false,"npm install -g @openai/codex","github.com/openai/codex"),h("opencode","opencode",false,false,"curl -fsSL https://opencode.ai/install | bash","opencode.ai")]
 : [h("claude","Claude Code",true,true,"curl -fsSL https://claude.ai/install.sh | bash","claude.com/claude-code"),h("codex","Codex",true,false,"npm install -g @openai/codex","github.com/openai/codex"),h("opencode","opencode",false,false,"curl -fsSL https://opencode.ai/install | bash","opencode.ai")];
const templates=[["rust","Rust","Cargo-aware personas and a review checklist."],["typescript","TypeScript","Node and web tooling, typecheck and test commands."],["python","Python","pytest, ruff and virtual-env commands."],["go","Go","go test, vet and module commands."],["monorepo","Monorepo","Personas per package and path-scoped checks."],["docs","Docs only","A writer persona and link checks."]].map(([id,title,summary])=>({id,title,summary}));
const forges=[{cli:"gh",title:"GitHub",installed:params.get("forge")==="yes",signed_in:params.get("forge")==="yes"},{cli:"glab",title:"GitLab",installed:false,signed_in:false}];
const repos=["purlis/purlis","purlis/purlis-plane","acme/api-gateway","acme/web-storefront","aaron/dotfiles","aaron/notes"].map((n,i)=>({kind:"github",host:"github.com",login:"aaron",path:n,owner:n.split("/")[0],name:n.split("/")[1],description:i===0?"The window for your coding agents":"",url:`https://github.com/${n}.git`,project:i<2}));
export function install(){
 const step=params.get("step")??"welcome"; const door=params.get("door");
 const signing=params.get("signing")==="1";
 mockIPC((cmd)=>{
  switch(cmd){
   case "plane_at_launch": return {plane:null,from:null,why:null};
   case "recent_planes": return params.get("recents")==="1"?{planes:[{path:"/Users/aaron/work/acme-platform",name:"acme-platform",approved:true},{path:"/Users/aaron/IdeaProjects/purlis",name:"purlis",approved:true},{path:"/Users/aaron/oss/charter-notes",name:"charter-notes",approved:false}],dropped:[],gone:params.get("gone")==="1"?[{path:"/Volumes/Backup/old-project",name:"old-project",reason:"It is not there any more.",why:"It is not there any more.",said:"It is not there any more."}]:[],forgetful:null}:{planes:[],dropped:[],gone:[],forgetful:null};
   case "onboarding_progress": return {step,door,signing_in:signing,git_missing:step==="git",made:step==="harness"?{plane:"/p",cwd:"/p",workspace:"main"}:null,finished_once:params.get("finished")==="1"};
   case "git_presence": return step==="git"?{kind:"missing",install:{kind:"developer-tools"}}:{kind:"present"};
   case "first_run_found": return {harnesses,forges,templates,local_models:[]};
   case "harness_setup_found": return {harnesses,local_models:[]};
   case "forge_accounts": case "forge_account_list": return params.get("forge")==="yes"?[{kind:"github",host:"github.com",login:"aaron"}]:[];
   case "forge_repo_list": return params.get("forge")==="yes"?{repos,reaches:[{kind:"github",host:"github.com",login:"aaron",listed:6,trouble:null}]}:{repos:[],reaches:[{kind:"github",host:"github.com",login:null,listed:0,trouble:null},{kind:"gitlab",host:"gitlab.com",login:null,listed:0,trouble:null}]};
   case "opened_chats": case "chat_states": case "chats_that_would_not_start": case "running_sessions": return [];
   default: return null;
  }
 });
}
