import assert from 'node:assert/strict';
import {spawn,execFileSync} from 'node:child_process';
import {mkdirSync,writeFileSync,readFileSync} from 'node:fs';
import {createServer} from 'node:net';
import {release} from 'node:os';
import {digest,assertCleanApplication} from './sync-evidence.mjs';

const output=process.argv[2];assert(output);mkdirSync(output,{recursive:false});
assertCleanApplication();
const head=execFileSync('git',['rev-parse','HEAD'],{encoding:'utf8',windowsHide:true}).trim();
const port=8136,probe=createServer();
await new Promise((resolve,reject)=>{probe.once('error',reject);probe.listen(port,'127.0.0.1',resolve);});
await new Promise(resolve=>probe.close(resolve));
const child=spawn(process.execPath,['scripts/live.mjs','--stand-ins'],{windowsHide:true,
  env:{...process.env,PORT:String(port),APP_DATA_DIR:`${output}/data`,LIBCLANG_PATH:''},stdio:['ignore','pipe','pipe']});
const chunks=[];child.stdout.on('data',bytes=>chunks.push(bytes));child.stderr.on('data',bytes=>chunks.push(bytes));
let spawnError;child.once('error',error=>{spawnError=error;});
let receipt;
try {
  let build;
  const end=Date.now()+180000;
  while(Date.now()<end){
    if(spawnError)throw spawnError;assert.equal(child.exitCode,null,'Owned live launcher exited before readiness');
    try{const response=await fetch(`http://127.0.0.1:${port}/build`,{signal:AbortSignal.timeout(1000)});if(response.ok){build=await response.json();break;}}catch{}
    await new Promise(resolve=>setTimeout(resolve,250));
  }
  assert(build,'Live launcher did not become ready');assert.equal(build.head,head);
  assert(Number.isSafeInteger(build.pid)&&build.pid>0);
  const parent=execFileSync('powershell',['-NoProfile','-Command',`(Get-CimInstance Win32_Process -Filter 'ProcessId=${build.pid}').ParentProcessId`],{encoding:'utf8',windowsHide:true}).trim();
  assert.equal(Number(parent),child.pid,'Answering API must be the owned launcher child');
  const request=async(method,path)=>{
    const response=await fetch(`http://127.0.0.1:${port}${path}`,{method});return {status:response.status,body:await response.json()};
  };
  const library=await request('GET','/library/data');assert.equal(library.status,200);assert.deepEqual(library.body.videos,[]);
  const privacy=await request('POST','/privacy/open');assert.equal(privacy.status,403);assert.equal(privacy.body.error,'native_required');
  receipt={source_revision:head,observed_at:new Date().toISOString(),environment:{os:release(),node:process.version,stand_ins:true,
    binary_sha256:digest(readFileSync('target/debug/vto-web.exe')),compiled_head:build.head,owned_launcher_pid:child.pid,owned_server_pid:build.pid},observations:[
      {name:'actual live command serves its own current-head core',pass:true,actual:{compiled_head:build.head,owned_process:true}},
      {name:'isolated live data starts with an empty library',pass:true,actual:{status:library.status,videos:library.body.videos.length}},
      {name:'HTTP cannot invoke the native privacy action',pass:true,actual:{status:privacy.status,error:privacy.body.error}},
    ]};
} finally {
  if(child.exitCode===null&&!spawnError){
    execFileSync('taskkill',['/PID',String(child.pid),'/T','/F'],{windowsHide:true,stdio:'ignore'});
    await new Promise(resolve=>child.exitCode!==null?resolve():child.once('exit',resolve));
  }
  writeFileSync(`${output}/launcher.log`,Buffer.concat(chunks));
}
await assert.rejects(()=>fetch(`http://127.0.0.1:${port}/build`,{signal:AbortSignal.timeout(1000)}));
receipt.observations.push({name:'owned live process tree stops and releases the API port',pass:true,actual:true});
writeFileSync(`${output}/receipt.json`,JSON.stringify(receipt,null,2)+'\n');
console.log(JSON.stringify({head,observations:receipt.observations.length,owned_process_tree_closed:true}));
