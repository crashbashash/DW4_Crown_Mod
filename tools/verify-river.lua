be = {}
calls = {}
colours = {}
generated = {}
turretPositions = {}
closed = {}
modelseq = {}
kind = 70108
guard = 0
hitcheck = 1
damagevalue = 1
turretSlot = 0
turretHp = 100
tame = 0
tech = 0
function be:sd(group, model, slot, command, param)
 if model == "g_numepodz01" and command == "GENERATE" then turretPositions[slot] = param end
 if model == "i_rareicon" then
  if command == "GENERATE" then generated[slot] = param end
  if strsub(command,1,7) == "SQ_OPEN" then colours[slot] = command end
  if command == "SQ_CLOSE" then closed[slot] = 1 end
 end
 if model == "g_boatz01" then modelseq[command] = 1 end
 if group == "PLAYER" and command == "SQ_DDMG" then failed = 1 end
end
function be:MDLgetBulletKind() return kind end
function be:MDLisSeqName() return guard end
function be:BLPisHitCheck() return hitcheck end
function be:BLPgetDamageValue() return damagevalue end
function be:MDLsetSeq(seq) lastseq = seq end
function be:MDLgetRamNo() return turretSlot end
function be:MDLsetDMGTurn() end
function be:MDLaddHp(v) turretHp = turretHp + v end
function be:MDLgetHp() return turretHp end
function funcTechWazaHitCheck() return tech end
function funcTameWazaHitCheck() return tame end
function funcStartLayerSet() end
function funcPlayerSet() end
function be:SYSgetLightMode() return 0 end
function be:FLGisOn() return 1 end
function be:SYSisJoinPlayer(n) if n == 0 then return 1 else return 0 end end
function be:MATisPlayer() return 1 end
function check(v,message) if not v then error(message) end end

original_dofile = dofile
function dofile(path)
 local result, message = original_dofile(path)
 if result == nil then error(path .. ": " .. message) end
 return result
end

for _, mode in {"yellow","random"} do
 check(river_audit_root, "set river_audit_root to DW4_RIVER_AUDIT_DIR")
 local base = river_audit_root .. "/" .. mode .. "/"
 g_crownRiverColourRng = nil
 g_lFLG24 = 0
 generated = {} colours = {} closed = {}
 dofile(base .. "start.lua")
 dofile(base .. "matb07.lua")
 local n = 0
 for slot = 56, 63 do
  check(generated[slot] ~= nil,"missing turret crown")
  local _,_,tx,ty,tz = strfind(turretPositions[slot-56],"V<([^,]+),([^,]+),([^>]+)>")
  local _,_,cx,cy,cz = strfind(generated[slot],"V<([^,]+),([^,]+),([^>]+)>")
  check(tonumber(cx)==tonumber(tx) and tonumber(cz)==tonumber(tz),"crown drifted from turret")
  check(tonumber(cy)==tonumber(ty)+5,"crown height is not close to turret")
  local rank = tonumber(strsub(colours[slot],8))
  check(rank >= 1 and rank <= 5,"invalid crown colour")
  if mode == "yellow" then check(rank == 5,"default turret crown not yellow") end
  n = n + 1
 end
 check(n == 8,"wrong crown count")
 if mode == "yellow" then check(g_crownRiverColourRng == nil,"yellow mode advanced RNG") end
 if mode == "random" then
  local state = 52719
  local seen = {}
  for slot = 56, 63 do
   state = mod(state * 25173 + 13849,65536)
   local rank = mod(state,5) + 1
   check(colours[slot] == "SQ_OPEN0" .. rank,"wrong private RNG rank")
   seen[rank] = 1
  end
  local count = 0
  for k,v in seen do count=count+1 end
  check(count > 1,"random colours all identical")
  check(g_crownRiverColourRng == state,"private RNG mismatch")
 end
 -- The trigger may be called again without generating anything.
 local prior = g_crownRiverColourRng
 dofile(base .. "matb07.lua")
 check(prior == g_crownRiverColourRng,"inactive trigger advanced crown RNG")
 for slot = 0, 7 do
  turretSlot = slot turretHp = 100 tame = 0 tech = 0 closed = {}
  dofile(base .. "hit_g_numepodz01.lua")
  check(turretHp == 50 and closed[slot+56] == nil,"first turret hit hid crown")
  dofile(base .. "hit_g_numepodz01.lua")
  check(turretHp == 0 and closed[slot+56] == 1,"destroyed turret kept crown")
  turretHp = 100 tame = 1 closed = {}
  dofile(base .. "hit_g_numepodz01.lua")
  check(closed[slot+56] == 1,"charged destruction kept crown")
  turretHp = 100 tame = 0 tech = 1 closed = {}
  dofile(base .. "hit_g_numepodz01.lua")
  check(turretHp == 100 and closed[slot+56] == nil,"ignored tech hit hid crown")
 end
 turretSlot = 0
 for party = 1, 4 do
  g_pCount = party
  local amounts = {5,4,3,2}
  local t1 = {75,76,76,74}
  local t2 = {50,52,52,50}
  local t3 = {25,28,28,24}
  local t4 = {0,0,1,0}
  g_dmgnum = amounts[party]
  g_hpnum1 = t1[party] g_hpnum2 = t2[party] g_hpnum3=t3[party] g_hpnum4=t4[party]
  g_boathp = 100 g_meta = 100 failed=0 modelseq={}
  kind=70108 guard=0 hitcheck=1 damagevalue=1
  local hits=0
  while failed == 0 do
   local previous=g_boathp
   dofile(base .. "player-hit.lua")
   check(g_boathp == max(0,previous - g_dmgnum * 3),"incorrect missile damage")
   check(g_meta >= 0,"negative boat meter")
   hits=hits+1 check(hits <= 17,"boat survives lethal missile damage")
  end
  check(hits == ceil((100-g_hpnum4)/(g_dmgnum*3)),"wrong lethal missile count")
  for _, attack in {0,70107,70109,10001} do
   kind=attack g_boathp=100 failed=0
   dofile(base .. "player-hit.lua")
   check(g_boathp == 100-g_dmgnum,"non-missile damage changed")
  end
  kind=70108 g_boathp=100 guard=1
  dofile(base .. "player-hit.lua") check(g_boathp==100,"guarded missile damaged boat")
  guard=0 hitcheck=0
  dofile(base .. "player-hit.lua") check(g_boathp==100,"miss damaged boat")
  hitcheck=1 damagevalue=0
  dofile(base .. "player-hit.lua") check(g_boathp==100,"zero damage hit damaged boat")
  damagevalue=1
 end
 print("PASS",mode,"eight crown slots, colours, death cleanup, missile-only damage, four party sizes and lethal overshoot")
end
