// /stage-class.js: die Klasse der Startseite vor dem ersten Bild (HUM-418), aus classes.mjs.
import type { APIRoute } from 'astro';

import { headClassScript } from '../scripts/stage/head-class.mjs';

export const GET: APIRoute = () =>
  new Response(headClassScript(), { headers: { 'Content-Type': 'text/javascript; charset=utf-8' } });
