import {createServer} from 'node:http';
import {readFileSync} from 'node:fs';
const media=readFileSync('src-tauri/tests/fixtures/library-video.mp4');
const html=`<!doctype html><meta charset="utf-8"><title>Saved lecture fixture</title><style>
html,body{margin:0;background:#000;color:white;font-family:system-ui} .html5-video-player{position:relative;width:100vw;height:100vh}
video{width:100%;height:100%;object-fit:contain}.html5-video-player:fullscreen{width:100vw;height:100vh}</style>
<div class="html5-video-player"><video src="/clip.mp4" controls preload="auto" playsinline></video></div>`;
createServer((req,res)=>{
 if(req.url==='/saved.mp4'){res.writeHead(200,{'content-type':'text/html; charset=utf-8'});res.end(html);return;}
 if(!['/clip.mp4','/other.mp4'].includes(req.url)){res.writeHead(404);res.end();return;}
 const range=/^bytes=(\d+)-(\d*)$/.exec(req.headers.range??'');
 const start=range?Number(range[1]):0,end=range?.[2]?Math.min(Number(range[2]),media.length-1):media.length-1;
 res.writeHead(range?206:200,{'content-type':'video/mp4','accept-ranges':'bytes','content-length':end-start+1,
  'access-control-allow-origin':'*',...(range?{'content-range':`bytes ${start}-${end}/${media.length}`}:{})});
 res.end(media.subarray(start,end+1));
}).listen(8159,'127.0.0.1',()=>console.log('Local saved lecture fixture: 8159'));
