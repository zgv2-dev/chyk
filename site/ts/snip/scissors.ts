// The scissors cursor, drawn on the canvas so it can turn to face the way the pointer moves.
// The blade tips sit on the pointer; `open` runs from 0 (shut) to 1 (wide open).

export const drawScissors = (ctx: CanvasRenderingContext2D, x: number, y: number, angle: number, open: number) => {
  const spread = 2 + open * 7; // half the gap between the blade tips
  ctx.save();
  ctx.translate(x, y);
  ctx.rotate(angle);
  ctx.lineCap = "round";
  ctx.lineJoin = "round";
  const shape = () => {
    ctx.beginPath();
    // Blades, from the pivot behind the pointer out to the tips.
    ctx.moveTo(-16, 0); ctx.lineTo(0, -spread);
    ctx.moveTo(-16, 0); ctx.lineTo(0, spread);
    // Handles cross over at the pivot and end in rings.
    ctx.moveTo(-16, 0); ctx.lineTo(-25, spread * 0.55 + 4);
    ctx.moveTo(-16, 0); ctx.lineTo(-25, -spread * 0.55 - 4);
    ctx.moveTo(-25.5 + 5, spread * 0.55 + 6.5); ctx.arc(-25.5, spread * 0.55 + 6.5, 5, 0, Math.PI * 2);
    ctx.moveTo(-25.5 + 5, -spread * 0.55 - 6.5); ctx.arc(-25.5, -spread * 0.55 - 6.5, 5, 0, Math.PI * 2);
  };
  shape();
  ctx.strokeStyle = "rgba(0,0,0,.85)";
  ctx.lineWidth = 4.5;
  ctx.stroke();
  shape();
  ctx.strokeStyle = "#4dff9b";
  ctx.shadowColor = "rgba(77,255,155,.7)";
  ctx.shadowBlur = 8;
  ctx.lineWidth = 2;
  ctx.stroke();
  // The pivot screw.
  ctx.shadowBlur = 0;
  ctx.fillStyle = "#d9ffe9";
  ctx.beginPath(); ctx.arc(-16, 0, 1.6, 0, Math.PI * 2); ctx.fill();
  ctx.restore();
};
