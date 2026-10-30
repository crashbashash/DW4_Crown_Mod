-- Run with the Lua 4.0.1 runtime, after setting pharaoh_audit_dir to a
-- directory exported by retail_pharaoh_summons_follow_selected_families.
function check(value, message) if not value then error(message) end end
be = {}
local old_dofile = dofile
function run(name)
 local value, message = %old_dofile(pharaoh_audit_dir .. "/" .. name)
 if value == nil then error(message) end
end
function be:MDLisCallName(name) if name == action then return 1 else return 0 end end
function be:SYSgetDifficulty() return difficulty end
function be:SYSgetRandom(n)
 rng = mod(rng * 25173 + 13849, 65536)
 rolls = rolls + 1
 return mod(rng, n)
end
function be:MDLisLive(model, slot)
 if live[model .. ":" .. slot] then return 1 else return 0 end
end
function be:MDLgetRamNo() return actorSlot or 0 end
function be:MDLgetBulletKind() return 90026 end
function be:MDLgetBulletName() return "ENSP" end
function be:MDLsetSeq(seq) mummySeq = seq end
function be:MDLgetMHp() return 100 end
function be:MDLaddHp(hp) mummyHealing = hp end
function be:MDLsetDMGTurn() end
function be:MDLgetPosStr() return "V<10,20,30>" end
function be:MDLgetRotStr() return "V<0,90,0>" end
function be:sd(group, model, slot, command, param)
 if model == "e_pharaoh" then
  check(command ~= "GENERATE", "recursive Pharaohmon summon")
  lastseq = command return
 end
 check(group == "MODELCTRL" and command == "GENERATE", "unexpected summon command")
 check(model ~= "e_pharaoh", "recursive Pharaohmon summon")
 local key = model .. ":" .. slot
 check(not live[key], "summon overwrote live actor")
 check(allowed[model] ~= nil and slot < allowed[model], "summon outside allocated stage pool")
 check(strfind(param,"V<10,20,30>/V<0,90,0>",1,1), "lost summoner position/rotation")
 local _, _, row = strfind(param,"/I<(%d+)>")
 check(rows[tonumber(row)] == difficulty, "wrong encounter row/difficulty")
 check(model ~= previousModel, "consecutive summons repeated a family")
 previousModel = model
 live[key] = 1
 seen[model] = 1
 generated = generated + 1
end
function invoke(name) action = name run("cal_e_pharaoh.lua") end
function count(t) local n = 0 for _, _ in t do n = n + 1 end return n end

run("metadata.lua")
for tier = 0, 2 do
 difficulty = tier rng = 12345 rolls = 0 generated = 0
 previousModel = nil
 live = {} seen = {} g_crownPharaohModels = nil g_crownPharaohRams = nil g_crownPharaohStage = nil
 g_magic = nil g_switch = nil
 invoke("check") check(lastseq == "SQ_ITEM1", "empty pool did not request summons")
 for slot = 1, 12 do
  invoke("gen")
  check(generated == slot and rolls == slot, "each summon needs a fresh roll")
 end
 invoke("check") check(lastseq == "SQ_ITEM3", "full pool not detected")
 invoke("gen") check(generated == 12 and rolls == 12, "full pool spawned more enemies")
 for round = 1, 20 do
  live = {}
  for slot = 1, 12 do invoke("gen") end
  check(count(live) == 12, "summons not replenished")
 end
 check(rolls == 252 and generated == 252, "missing per-enemy rolls")
 check(count(seen) >= 2, "summon choice never changed family")
 -- Refill one slot while all the other summons stay alive.
 local killed
 for key, _ in live do if not killed then killed = key end end
 live[killed] = nil
 local previous = generated
 invoke("gen")
 check(generated == previous + 1 and count(live) == 12, "partial pool refill failed")
 invoke("mck") invoke("magic") invoke("reset") invoke("stset")
end
if mummy_base then
 actorSlot = mummy_base + 7
 g_magic = 0 g_switch = 0 g_sw7 = 0
 action = "request" run("mag_e_mummy.lua")
 check(g_sw7 == 1 and g_magic == 1, "summoned Mummymon lost its logical request index")
 g_switch = 1
 run("hit_e_mummy.lua")
 check(g_sw7 == 2 and mummyHealing == 50 and mummySeq == "SQ_RECV", "summoned Mummymon lost its hit/magic coordination")
 action = "reset" run("mag_e_mummy.lua")
 check(g_sw7 == 0, "summoned Mummymon request did not reset")
 actorSlot = 2 g_magic = 0 g_switch = 0 g_sw2 = 0
 action = "request" run("mag_e_mummy.lua")
 check(g_sw2 == 1, "ordinary Mummymon actor index changed")
end
-- Stale tracking from a different stage must not address its old actors.
actorSlot = 0 live = {} previousModel = nil generated = 0
mummy_base = nil g_crownPharaohStage = -1
g_crownPharaohModels = {[0] = "missing_model"}
g_crownPharaohRams = {[0] = 999999}
invoke("gen")
check(generated == 1, "stage change did not reset summon tracking")
print("Pharaohmon: all difficulties, per-enemy rolls, bounded pool, live-slot safety and partial refill passed")
