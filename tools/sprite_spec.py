"""스프라이트 시트 규격 — 앱(src/manifest.rs REQUIRED_ANIMS)과 같은 순서를 유지한다."""

import math

FRAME = 128  # 그림 좌표의 한 변 (draw_*는 모두 이 좌표로 그린다). 시트 한 칸의 실제 px는 frame_px
# roll 공: 이 점을 축으로 웹뷰가 회전시킨다 (ui/style.css transform-origin 50% 68.75%)
ROLL_CENTER = (64, 88)
ROLL_RADIUS = 38
ROLL_SPEED_MULT = 2.0  # brain behavior Move::Roll.speed_mult()

# (동작, 기본 프레임 수, fps, 반복, 앞모습)
ANIMS = [
    ("walk", 4, 8, True, False),
    ("run", 4, 12, True, False),
    ("roll", 2, 4, True, False),
    ("idle", 2, 2, True, False),
    ("look", 4, 3, True, False),
    ("sleep", 2, 1, True, False),
    ("hop", 3, 10, False, False),
    ("fall", 2, 8, True, False),
    ("dragged", 2, 4, True, False),
    ("land", 2, 10, False, False),
    ("sit_front", 4, 4, True, True),
    ("wave", 4, 6, True, True),
    ("tilt", 3, 3, True, True),
    ("belly", 4, 4, True, True),
    ("stretch", 3, 3, False, True),
    ("heart", 3, 4, True, True),
    ("tickle", 3, 8, True, True),
    ("annoyed", 2, 2, True, False),
    ("hungry", 3, 3, True, True),
    ("eat", 4, 6, True, True),
    ("level_up", 3, 6, False, True),
]

# 성장 단계 (Rust care::Stage) — 종마다 sheet_<stage>.png
STAGES = ["baby", "kinder", "child", "teenager", "adult"]
# 화면 표시 배율 (Rust Stage::scale)
STAGE_SCALE = {"baby": 0.45, "kinder": 0.60, "child": 0.78, "teenager": 1.00, "adult": 1.15}
# 청소년 때 이미 128px보다 크게 보이는 동물 (pet.json stage_size >= 1) — 어른과 함께 2배 시트
HIRES_TEENAGER = {"santa", "dino_green", "dino_yellow", "hagrid"}


def sheet_128(path):
    """시험용: 시트(가로 4칸)를 128 칸으로 읽는다 — 그림 좌표로 재는 검사는 칸 크기와 상관없다. 2배 칸은 2×2 평균이라 1px 여백도 그대로."""
    from PIL import Image
    im = Image.open(path)
    return im.reduce(im.width // (4 * FRAME)) if im.width > 4 * FRAME else im


def frame_px(pid, stage):
    """동글 시트 한 칸의 px: 크게 보이는 단계는 벡터에서 2배로 다시 그린다 (화면 크기는 그대로 — 앱은 칸을 표시 크기에 맞춘다).
    pet.json frame_sizes에 적히고, head·neck 표도 이 칸의 px로 적는다."""
    return FRAME * 2 if stage == "adult" or (stage == "teenager" and pid in HIRES_TEENAGER) else FRAME


# 특정 동물만 가진 동작 (Rust manifest::OPTIONAL_ANIMS) — 필수 동작 행 뒤에 이어 붙인다
OPTIONAL_ANIMS = [
    ("arch", 3, 6, True, False),
    ("huff", 3, 4, True, True),
    # 뒷모습 (고양이·산타): front=False지만 앱은 반전하지 않는다 (Rust Task 4b에서 반전 안 함으로 처리)
    ("climb", 4, 6, True, False),
    ("slide", 2, 8, True, False),
    # 폴짝 동물 (locomotion hop — 지금 번들엔 없음, 개구리는 v0.7.1에 빠짐): 크게 뛰기 (옆모습, 한 번) — 작은 폴짝은 필수 동작 hop
    ("bigjump", 4, 8, False, False),
    # 모든 동물: 위를 올려다보기 (옆모습, 고개 들기 · 쭉 · ✧) — 머리 위에 든 음식, 가끔 쉬다가·걷다가, 커서가 머리 위에 머물 때
    ("lookup", 3, 3, True, False),
    # 공룡: 불 뿜기 (옆모습, 흡 · 퐁 · 화르륵 · 흐뭇 — 한 번) · 크앙 (옆모습, 고개 들고 소리 물결) · 제자리 쿵쾅쿵쾅 (옆모습, 먼지 퐁퐁)
    ("fire", 4, 4, False, False),
    ("roar", 3, 4, True, False),
    ("stomp", 4, 6, True, False),
    # 흐물이: 으악! 등을 돌려 두 팔 번쩍(만세) 들고 줄행랑 (옆모습, 반복 — 코어 State::Panic, 친구 동물과 마주치면 반쯤)
    ("panic", 4, 12, True, False),
    # 흐물이: 창 옆면에 한 손으로 매달려 다른 팔로 흐물흐물 손 흔들기 · 밧줄 타듯 손을 번갈아 오르내리기 (옆모습, 창은 오른쪽 — 코어 State::Rig)
    ("hang", 4, 6, True, False),
    ("rope", 4, 8, True, False),
    # 흐물이 🤹 물건 들어 던지기 (코어 State::Juggle — 물건은 앱의 물건 창, 손 자리는 pet.json grip): 쪼그려 잡아 머리 위로 번쩍 (한 번) ·
    # 든 채 서 있기 · 인 채 걷기 · 젖혔다가 휙 (한 번, 셋째 칸에 놓는다) · 거대한 것에 끙차 (덜덜) · 배달 도착 앞모습 짠 → 휙 떨굼 (한 번) · 털썩 주저앉기
    ("lift", 4, 8, False, False),
    ("hold", 2, 3, True, False),
    ("carry", 4, 8, True, False),
    ("throw", 4, 10, False, False),
    ("strain", 4, 12, True, False),
    ("present", 4, 3, False, True),
    ("slump", 2, 2, True, False),
    # 흐물이 📌 작업표시줄 놀이 (코어 brain_taskbar.rs — 장난감은 앱의 물건 창, 손 자리는 pet.json grip): 쪼그려 앞 바닥을 잡고 끙차 당기기 (반복) ·
    # 머리 위의 장난감을 내려 바닥 속으로 꾹 밀어 넣기 (한 번, 넷째 칸 = STUFF_RELEASE 0.5초에 쏙)
    ("yank", 4, 6, True, False),
    ("stuff", 4, 6, False, False),
    # 흐물이 더 놀기 (v0.6.3): 🎣 커서 끝에 두 손으로 대롱대롱 (손 = grip, 웹뷰가 그 점을 축으로 흔든다) · 🫣 창 옆면 뒤에서 머리만 쏙 (창은 오른쪽,
    # 옆면 오른쪽은 그리지 않는다) · 🕺 연타 춤 둘 (앞모습 — 둘째는 더 신나게) · ⚾ 날아온 것 받기 (한 번, 손 = grip) · 🖍️ 쪼그려 분필 낙서 ·
    # ⌨️ 바닥의 작은 키보드 앞에 앉아 가끔 만세 (cheer_type) · 같이 타자 (type — 쉼 · 왼손 톡 · 오른손 톡, 칸은 코어 render.tap이 고른다)
    ("hang_cursor", 4, 6, True, False),
    ("peek", 4, 3, True, False),
    ("dance", 4, 8, True, True),
    ("dance2", 4, 10, True, True),
    ("catch", 4, 10, False, False),
    ("draw", 4, 6, True, False),
    ("cheer_type", 4, 10, True, True),
    ("type", 3, 6, True, True),
    # 흐물이: 머리 위에 인 채 다다다 달리기 (친구에게 들고 가는 길·화면 끝을 들고 나가고 들어올 때 — 사용자: 걷지 말고 달려!)
    ("carry_run", 4, 12, True, False),
    # 🐱 고양이 툭툭 (v0.6.3, 코어 brain_knock.rs): 고개 숙여 앞발을 들었다가 앞으로 툭 (한 번 = 0.5초, 셋째 칸에 민다)
    ("paw", 4, 8, False, False),
    # 🐿️ 다람쥐 볼 빵빵 (v0.6.3, 코어 brain_stash.rs): 볼이 빵빵한 채 총총 걷기 · 앉기, 땅을 파고 묻기 (흙이 튄다)
    ("cheeks_walk", 4, 8, True, False),
    ("cheeks_idle", 2, 2, True, False),
    ("dig", 4, 8, True, False),
    # 🕷️ 거미인간 (코어 brain_web.rs — 줄 끝 손 자리는 pet.json grip): 앞다리를 쭉 뻗어 줄 쏘기 (한 번, 마지막 칸 유지) ·
    # 한 손을 머리 위로 들어 줄에 매달려 그네 · 영웅 착지 (쪼그려 한 손 짚고 다리 하나 쭉 — 한 번) · 두 손으로 줄을 잡고 끌어당기기
    ("shoot", 3, 10, False, False),
    ("web_swing", 2, 4, True, False),
    ("hero_land", 3, 8, False, False),
    ("web_pull", 4, 8, True, False),
    # 🕷️ 🌀 줄 휘두르기: 버티고 서서 한 팔을 머리 위로 들어 줄 끝의 물건을 빙빙 (손목으로 작은 원 — 3칸 반복)
    ("web_whirl", 3, 10, True, False),
    # 🕷️ 🙃 거꾸로 매달리기: 웅크린 채 뒤집혀 위로 뻗은 줄을 두 손·두 발로 꼭 잡고 살랑 (3칸 반복 — 줄이 붙는 맨 위 손발 = grip)
    ("hang_down", 3, 4, True, False),
    # 🦇 박쥐인간 (코어 brain_bike.rs): 🏍️ 오토바이 타고 달리기 (바퀴가 굴러가는 3칸) · 앞바퀴 든 윌리 · 오토바이째 점프 · 급정거 드리프트 ·
    # 🔦 박쥐 신호 조명 켜기 (한 번, 마지막 칸 유지 — 빛줄기는 앱의 덮개 창)
    ("ride", 3, 10, True, False),
    ("wheelie", 3, 10, True, False),
    ("bike_jump", 2, 6, True, False),
    ("drift", 2, 8, True, False),
    ("signal", 3, 4, False, False),
    # 🦸 슈퍼인간 (코어 brain_soar.rs): ✈️ 몸을 눕혀 주먹을 앞으로 내민 비행 (망토 펄럭 3칸) · 두 주먹 쭉 빠른 비행 (속도선) ·
    # 🔄 공중제비 한 바퀴 (4칸 — 코어가 한 바퀴 시간만큼 틂) · 🫧 똑바로 떠서 둥실 (이륙·사뿐 착지·위아래 친구 화면)
    ("fly", 3, 10, True, False),
    ("fly_fast", 3, 14, True, False),
    ("fly_flip", 4, 12, True, False),
    ("hover", 3, 6, True, False),
    # 🧙 마법사 (해리포터·헤르미온느 — 코어 brain_broom.rs·brain_spell.rs·brain_study.rs): 🧹 빗자루에 걸터앉아 날기 (로브·목도리 펄럭 3칸) ·
    # ✨ 지팡이 휙 (한 번, 마지막 칸 유지 — 지팡이 끝 = pet.json grip) · 📖 바닥에 앉아 책 읽기 (책장 넘김) · 🙋 앞모습 손 번쩍 "I know!" ·
    # 👜 구슬 가방 들고 걷기 · 커진 가방 질질 끌기 · 가방에 쏙 넣기 (가방은 앱이 손 자리 grip에 그림 — 시트엔 없음)
    ("broom", 3, 8, True, False),
    ("cast", 3, 8, False, False),
    ("read", 4, 2, True, False),
    ("raise_hand", 2, 4, True, True),
    ("bag_walk", 4, 8, True, False),
    ("bag_drag", 4, 6, True, False),
    ("bag_stuff", 2, 4, False, False),
    # 🧔 해그리드 (코어 brain_hagrid.rs — 용은 brain_broom.rs가 빗자루처럼 태움): 🐉 용 등에 앉아 날기 (날개 퍼덕 3칸) ·
    # 🏍️ 사이드카 오토바이 타기 (바퀴 3칸 — 사이드카 자리 = grip) · 오토바이째 붕 날기 · 🤗 펫을 가슴에 안고 서기 · 안고 걷기 (손 = grip) ·
    # 🖐️ 쓰담쓰담 · 🥚 앉아서 용알 품기 (알·아기 용은 앱 덮개 창 — 손 = grip) · 🪨 코트에서 락 케이크를 꺼내 바닥에 내려놓기 (한 번)
    ("dragon", 3, 8, True, False),
    ("sidecar", 3, 10, True, False),
    ("sidecar_fly", 2, 6, True, False),
    ("hug", 2, 2, True, False),
    ("hug_walk", 4, 8, True, False),
    ("pat", 3, 6, True, False),
    ("egg", 4, 2, True, False),
    ("offer", 3, 3, False, False),
    # 🛡️ 무쇠인간 (코어 Special::SuitUp): 🪖 투구 장착 — 맨얼굴 → 빨간 반쪽 투구 둘이 양옆에서 미끄러져 와 → 착 닫힘 → 렌즈 켜짐
    # (앞모습, 4칸 한 번, 마지막 칸 유지). 🚀 제트팩 날기는 🦸 슈퍼인간의 fly·fly_fast·fly_flip·hover를 그대로 씀
    ("suit_up", 4, 4, False, True),
]

# 말풍선 대사 (spec §2.6) — 먹이·성장 대사(hungry/full/level_up)는 모두 같다
_CARE_SPEECH = {"hungry": ["hungry...", "food?"], "full": ["full~"], "level_up": ["I grew up!!"]}
# 공룡 둘 (불 뿜기·크앙·쿵쾅 말풍선 roar/fire/stomp)
_DINO_SPEECH = {"chatter": ["rawr!", "♪"], "tickle": ["hehe!"], "grumpy": ["grrr~"], "roar": ["RAWR!", "grrr~"], "fire": ["fwoosh~"],
                "stomp": ["stomp!"], **_CARE_SPEECH}
SPEECH = {
    "cat": {"chatter": ["meow!", "nya~"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "hiss": ["hiss!"], "ignore": ["..."],
            **_CARE_SPEECH},
    "squirrel": {"chatter": ["chk chk!", "nuts!", "zoom!"], "tickle": ["hehe!"], "grumpy": ["chk-chk!"], **_CARE_SPEECH},
    "rabbit": {"chatter": ["boing!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "pumpkin": {"chatter": ["trick or treat!", "boo~", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "rudolph": {"chatter": ["jingle~", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "santa": {"chatter": ["ho ho ho!", "merry~"], "tickle": ["ho ho!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "snowman": {"chatter": ["brrr~", "snow!", "♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "penguin": {"chatter": ["waddle~", "wheee!", "pip pip!"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    "heumul": {"chatter": ["wheee!", "zoom zoom!", "hup!"], "tickle": ["hehehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH},
    # 🕷️ 거미인간: 줄 쏘기 thwip · 거미집 web · 그네 web_swing · 영웅 착지 hero_land · 끌어오기 gotcha · 🌀 휘둘러 던지기 yeet · 🙃 거꾸로 매달려 인사 hang_hey ·
    # 🕷️🕷️🕷️ 멀티버스 모임 놀람 multiverse / 흩어지기 see_ya (코어 Speech)
    "spider": {"chatter": ["thwip!", "web time!", "hi-ya!"], "tickle": ["hehe!"], "grumpy": ["hmph!"], "thwip": ["thwip!"],
               "web": ["web time!", "spin spin~"], "web_swing": ["wheee!", "swing!", "woohoo!"],
               "hero_land": ["nailed it!", "hero landing!", "super landing!", "stuck it!", "like a pro", "boom!",
                             "still got it", "ground secured", "ta-da!", "whoosh… done"],
               "gotcha": ["gotcha!", "yoink!"], "yeet": ["yeet!", "catch!", "go long!"], "hang_hey": ["hey there", "sup?", "hi down there!"],
               "multiverse": ["multiverse?!", "wait… you too?", "no way!", "another me?!", "which one's me?"],
               "see_ya": ["see ya!", "later, me!", "bye, me!"], **_CARE_SPEECH},
    # 🦇 박쥐인간: 오토바이 타기 vroom · 윌리 wheelie · 점프·드리프트 bike_trick · 🔦 박쥐 신호 bat_signal · 오토바이로 놀러 감 to_the_bike ·
    # 💪 거대한 것도 번쩍 hero_lift (코어 Speech)
    "bat": {"chatter": ["I'm the night", "to the bike!", "justice!"], "tickle": ["hehe!"], "grumpy": ["hmph."],
            "vroom": ["vroom!", "to the bike!", "vroom vroom!"], "wheelie": ["yeah!", "wheelie!", "woo-hoo!"],
            "bike_trick": ["hup!", "skrrt!", "nailed it!"], "bat_signal": ["I'm the night", "signal's up!", "gotham calls"],
            "hero_lift": ["easy!", "light as air", "hup!"], **_CARE_SPEECH},
    # 🦸 슈퍼인간: 날아오르기 take_off · 친구 화면으로 날아감 rescue · 🔄 공중제비 air_flip · 사뿐 착지 soft_land ·
    # 💪 거대한 것도 한 손으로 번쩍 hero_lift (코어 Speech)
    "super": {"chatter": ["up, up and away!", "to the rescue!", "hi there!"], "tickle": ["hehe!"], "grumpy": ["hmph."],
              "take_off": ["up, up and away!", "whoosh!", "to the sky!"], "rescue": ["to the rescue!", "on my way!", "hang on!"],
              "air_flip": ["wheee!", "flip!", "ta-da!"], "soft_land": ["touchdown!", "soft landing", "and... landed"],
              "hero_lift": ["easy!", "one hand!", "light as air"], **_CARE_SPEECH},
    # 🧙 마법사 둘: 🧹 빗자루 broom_up·broom_land·broom_away (친구 화면으로) · ✨ 주문 leviosa·fizzle (실패) ·
    # 해리 expelliarmus·🟡 스니치 snitch·got_it · 헤르미온느 reparo·🙋 i_know·📖 study·👜 tidy_up·all_clean (코어 Speech)
    "harry": {"chatter": ["hi!", "Quidditch?", "mischief managed"], "tickle": ["hehe!"], "grumpy": ["hmph."],
              "broom_up": ["up we go!", "whoosh!", "to the skies!"], "broom_land": ["smooth landing", "and down!"],
              "broom_away": ["off to Hogwarts!", "see you!"], "leviosa": ["Wingardium Leviosa!"], "fizzle": ["Leviosaaa?", "huh?", "oops..."],
              "expelliarmus": ["Expelliarmus!"], "snitch": ["the Snitch!", "there it is!"], "got_it": ["Got it!", "Caught it!"],
              "popped_out": ["Phew!", "Phew! Thanks!"], "reunion": ["Hagrid!", "you're here!"],
              "fireside": ["Remember Hogwarts?", "Quidditch, anyone?", "best day ever"], "pester": ["psst! Hermione!", "play with me?", "boo!"],
              **_CARE_SPEECH},
    "hermione": {"chatter": ["hello!", "honestly...", "books!"], "tickle": ["hehe!"], "grumpy": ["honestly!"],
                 "broom_up": ["up we go!", "whoosh!", "to the skies!"], "broom_land": ["smooth landing", "and down!"],
                 "broom_away": ["off I go!", "back soon!"], "leviosa": ["It's Levi-O-sa!", "Wingardium Leviosa!"],
                 "fizzle": ["Leviosaaa?", "huh?", "oops..."], "reparo": ["Reparo!"], "i_know": ["I know!", "ooh! I know!", "I read about it!"],
                 "study": ["fascinating...", "hmm...", "shh!"], "tidy_up": ["tidy time!", "into the bag!"], "all_clean": ["Phew! All clean!"],
                 "in_you_go": ["In you go!", "Quick, in here!"], "reunion": ["Hagrid!", "Harry!"],
                 "fireside": ["Remember Hogwarts?", "I read about this!", "so cozy~"], "shh": ["Shh! I'm studying!"],
                 "oh_alright": ["Oh, alright!", "Fine, one game!"], **_CARE_SPEECH},
    # 🐍 볼드모트: ✨ 빗자루 broom_* · ⚡ stupefy · 🐍 nagini (Serpensortia) · 😈 hehehe · ⚔️ duel_call (결투 걸기) ·
    # 👻 디멘터: chatter는 숨소리, 🦌 쫓기면 repelled (코어 Speech)
    "voldemort": {"chatter": ["hehehe", "come here...", "Potter..."], "tickle": ["hehe... stop!"], "grumpy": ["silence!"],
                  "broom_up": ["up!", "hehehe"], "broom_land": ["hmm.", "landed."], "broom_away": ["I'll be back", "hehehe"],
                  "stupefy": ["Stupefy!"], "nagini": ["Serpensortia!", "Nagini..."], "hehehe": ["hehehe", "hehehe...", "kekeke"],
                  "duel_call": ["You again, Potter?", "Duel me!"], **_CARE_SPEECH},
    # 🧔 해그리드: 🐉 용 타기 broom_up·broom_land·broom_away · 🏍️ 오토바이 vroom · 사이드카 hop_in · 🤗 안기 cuddle · 🖐️ 쓰담 pat ·
    # 🥚 용알이 깨어남 hatch · 🪨 락 케이크 rock_cake · 🔥 모닥불 reunion·light_fire·fireside·douse · 📖 방해 pester (코어 Speech)
    # (해리: 🎒 가방에서 popped_out · 🔥 reunion·fireside · 📖 pester / 헤르미온느: 🎒 in_you_go · 🔥 reunion·fireside · 📖 shh·oh_alright)
    "hagrid": {"chatter": ["Yer a wizard!", "Blimey!", "Rock cake?"], "tickle": ["hoho!"], "grumpy": ["Oi!"],
               "broom_up": ["Up we go!", "Blimey!"], "broom_land": ["Good dragon!", "Whoa there!"], "broom_away": ["Off ter Hogwarts!"],
               "vroom": ["Brrm brrm!", "Hold on!"], "hop_in": ["Hop in!", "Room fer one!"], "cuddle": ["C'mere, you!", "Up yeh go!"],
               "pat": ["Good lad!", "There, there"], "hatch": ["Blimey!", "Aww, a baby!"], "rock_cake": ["Rock cake?", "Rock cake!"],
               "reunion": ["Harry! Hermione!", "Blimey, it's you!"], "light_fire": ["Fire's up!", "Nice an' warm!"],
               "fireside": ["Them were the days!", "Yer a wizard!", "Good times"], "douse": ["Off ter bed!", "Night, you two!"],
               "pester": ["Oi, Hermione!", "Whatcha readin'?"], **_CARE_SPEECH},
    # 🛡️ 무쇠인간: 🚀 제트팩 날기 take_off·rescue·air_flip·soft_land (🦸 슈퍼인간과 같은 코어 Speech) · 🪖 투구 장착 suit_up ·
    # 🛡️ 팔뚝 막기 tickle·grumpy · 🔋 충전·💗 하트는 혼잣말 chatter
    "musoe": {"chatter": ["Beep boop!", "Charging...", "Heart power!", "Beep beep!"], "tickle": ["Blocked!", "Beep! Hey!"],
              "grumpy": ["Shields up!", "Beep."], "take_off": ["Jets on!", "Liftoff!", "Beep! Up!"],
              "rescue": ["On my way!", "Jets on!"], "air_flip": ["Loop-de-loop!", "Wheee!"],
              "soft_land": ["Landed. Beep.", "Touchdown!"], "suit_up": ["Suit up!", "Helmet on!", "Beep! Ready!"],
              **_CARE_SPEECH},
    "dementor": {"chatter": ["hhhhh...", "...", "hhhh"], "tickle": ["hhh?"], "grumpy": ["HHHH!"], "repelled": ["hsss!", "HHH!"],
                 **_CARE_SPEECH},
    **{pid: dict(_DINO_SPEECH) for pid in ("dino_yellow", "dino_green")},
}
DEFAULT_SPEECH = {"chatter": ["♪"], "tickle": ["hehe!"], "grumpy": ["hmph!"], **_CARE_SPEECH}

# 성격 (pet.json personality) — 없는 동물은 적지 않는다
PERSONALITY = {"cat": "aloof", "squirrel": "playful", "pumpkin": "playful",
               "penguin": "playful", "heumul": "playful", "spider": "playful", "bat": "playful", "super": "playful", "musoe": "playful", "harry": "playful", "hermione": "friendly",
               "voldemort": "playful", "hagrid": "friendly",
               "rudolph": "friendly", "santa": "friendly", "snowman": "playful",
               "dino_yellow": "playful", "dino_green": "friendly"}

# 움직임 (pet.json locomotion) — 폴짝·나는 동물만 적는다 (없으면 걷는다). 👻 디멘터만 날고 나머지는 걷기 (개구리는 v0.7.1에 빠짐)
LOCOMOTION = {"dementor": "fly"}

# 사는 집 그림 (pet.json house) — 보통 집은 적지 않는다. 앱 ui/house/<house>_<stage>.png
HOUSE = {"pumpkin": "haunted", "rudolph": "xmas", "santa": "xmas", "snowman": "xmas",
         # 🏰 호그와트 성 (해리·헤르미온느·해그리드) · 🏚️ 쓰러져 가는 나무집 (볼드모트·디멘터)
         "harry": "castle", "hermione": "castle", "hagrid": "castle", "voldemort": "shack", "dementor": "shack"}

# 걷기·달리기 토막 중 깡총거리는 몫 (pet.json hop_share) — 걷는 동물 중 토끼 절반, 호박은 늘 통통 (폴짝 동물은 locomotion hop이라 늘 폴짝)
HOP_SHARE = {"rabbit": 0.5, "squirrel": 0.35, "pumpkin": 1.0, "rudolph": 0.3, "snowman": 0.5, "heumul": 0.5}

# roll을 돌리지 않는 동물 (pet.json roll에 spin 없음) — 산타는 구르는 대신 썰매를 타고, 눈사람은 배로 미끄러지고, 펭귄은 배로 쭉 미끄러진다
NO_SPIN = {"santa", "snowman", "penguin"}

# 가끔 머리 위에 작은 눈구름을 부르는 동물 (pet.json snow_cloud)
SNOW_CLOUD = {"snowman"}

# 장난칠 때 안경이 번쩍하는 동물 (pet.json glasses_flash — 앱이 커서 장난·친구 놀이·가끔 render.flash를 켠다)
GLASSES_FLASH = {"penguin"}

# 다른 동물과 마주치면 반쯤 깜짝 놀라 줄행랑치는 겁쟁이 (pet.json timid — 코어 State::Panic, 그림은 panic)
TIMID = {"heumul"}

# 기운이 넘치는 동물 (pet.json energetic — 걷기 대신 달리기를 더 자주, 창 위로 크게 뛰기도 더 자주)
ENERGETIC = {"heumul"}

# 창 옆면에 뱃사람처럼 매달려 손 흔들고 오르내리는 동물 (pet.json rigging — 코어 State::Rig, 그림은 hang·rope)
RIGGING = {"heumul"}

# 바닥의 물건·똥을 주워 머리 위로 들고 던지는 동물 (pet.json juggler — 코어 State::Juggle, 그림은 lift·hold·carry·throw·strain·present·slump)
JUGGLER = {"heumul", "bat", "super"}

# 작업표시줄(Dock)에서 열린 앱 아이콘 장난감을 끌어내 놀다가 도로 넣는 동물 (pet.json taskbar_play — 코어 brain_taskbar.rs, 그림은 yank·stuff)
TASKBAR_PLAY = {"heumul"}

# 🐱 창 윗면의 작은 물건을 툭툭 밀어 떨어뜨리는 동물 (pet.json knocker — 코어 brain_knock.rs, 그림은 paw)
KNOCKER = {"cat"}

# 🐿️ 바닥의 작은 물건을 볼에 넣어 묻는 동물 (pet.json stasher — 코어 brain_stash.rs, 그림은 cheeks_walk·cheeks_idle·dig)
STASHER = {"squirrel"}

# 🕷️ 줄을 쏘아 모서리에 거미집을 치고, 줄 그네를 타고, 영웅 착지를 하고, 물건·친구를 끌어오는 동물
# (pet.json web_slinger — 코어 brain_web.rs, 그림은 shoot·web_swing·hero_land·web_pull·web_whirl·hang_down)
WEB_SLINGER = {"spider"}

# 🦇 오토바이를 자주 타고, 윌리·점프·드리프트를 하고, 가끔 박쥐 신호 조명을 켜고, 친구 화면으로 오토바이 타고 오가는 동물
# (pet.json bike_rider — 코어 brain_bike.rs, 그림은 ride·wheelie·bike_jump·drift·signal)
BIKE_RIDER = {"bat"}

# 🦸 걷기 대신 바닥 조금 위를 낮게 날아다니고, 공중제비를 돌고, 디딤판 끝에서 멀리 활공해 사뿐 내려앉고, 친구 화면도 날아서 오가는 동물
# (pet.json super_flier — 코어 brain_soar.rs, 그림은 fly·fly_fast·fly_flip·hover)
SUPER_FLIER = {"super", "musoe"}  # 🛡️ 무쇠인간은 같은 날기를 제트팩으로

# 🧹 가끔 빗자루에 올라타 화면을 낮게·높게 날아다니고 친구 화면도 빗자루로 오가는 동물 (pet.json broom_rider — 코어 brain_broom.rs, 그림 broom)
BROOM_RIDER = {"harry", "hermione", "voldemort", "hagrid"}  # 해그리드는 빗자루 대신 용 (dragon 그림)

# ✨ 지팡이로 거는 주문 (pet.json spells — 코어 brain_spell.rs, 그림 cast): expelliarmus 튕겨 내기 · leviosa 둥둥 띄우기 · reparo 쓰레기 반짝 치우기
# · stupefy ⚡ 초록 빛줄기로 💫 기절 (볼드모트)
SPELLS = {"harry": ["expelliarmus", "leviosa"], "hermione": ["leviosa", "reparo"], "voldemort": ["stupefy"]}

# 🐍 나기니 부르기 · 😈 hehehe (pet.json dark_lord — 볼드모트, 그림 cast)
DARK_LORD = {"voldemort"}

# 👻 디멘터: 곁의 펫 시무룩·배고픔, 화면 구석 🥶 서리, 🦌 패트로누스에 도망 (pet.json dementor — locomotion fly)
DEMENTOR = {"dementor"}

# 🦌 디멘터가 오면 "Expecto Patronum!" 은빛 사슴 (pet.json patronus — 해리, 그림 cast)
PATRONUS = {"harry"}

# 🟡 골든 스니치를 빗자루로 쫓아가 잡는 동물 (pet.json seeker — 해리)
SEEKER = {"harry"}

# 📖 앉아서 책 읽기 · 🙋 "I know!" 손들기 (pet.json bookworm — 헤르미온느, 그림 read·raise_hand)
BOOKWORM = {"hermione"}

# 👜 끝없는 구슬 가방으로 바닥 물건 치우기 (pet.json bag_cleaner — 헤르미온느, 그림 bag_walk·bag_drag·bag_stuff, 가방은 앱이 그림)
BAG_CLEANER = {"hermione"}

# 🧔 해그리드: 🏍️ 사이드카 오토바이 (가끔 붕 날고 작은 펫을 태움) · 🤗 안아 들기·쓰담 · 🥚 용알 품기 · 🪨 락 케이크
# (pet.json gamekeeper — 코어 brain_hagrid.rs, 그림 HAGRID_ANIMS)
GAMEKEEPER = {"hagrid"}

# 🎨 몸 색을 고를 수 있는 동물 (pet.json tint + ink_<stage>.png 잉크 자리 — 앱이 그 자리만 고른 색으로 덮는다, ui/main.js)
TINTABLE = {"heumul"}

# 좋아하는 음식 (pet.json favorite_food)
FAVORITE_FOOD = {
    "cat": {"emoji": "🐟", "name": "생선"},
    "squirrel": {"emoji": "🌰", "name": "도토리"},
    "rabbit": {"emoji": "🥕", "name": "당근"},
    "pumpkin": {"emoji": "🍬", "name": "사탕"},
    "rudolph": {"emoji": "🍎", "name": "사과"},
    "santa": {"emoji": "🥛", "name": "우유"},
    "snowman": {"emoji": "🍧", "name": "빙수"},
    "dino_yellow": {"emoji": "🍉", "name": "수박"},
    "dino_green": {"emoji": "🥦", "name": "브로콜리"},
    "penguin": {"emoji": "🦐", "name": "새우"},
    "heumul": {"emoji": "🍭", "name": "막대사탕"},
    "spider": {"emoji": "🪰", "name": "파리"},
    "bat": {"emoji": "🍎", "name": "사과"},
    "super": {"emoji": "🥛", "name": "우유"},
    "musoe": {"emoji": "🔋", "name": "배터리"},
    "harry": {"emoji": "🍬", "name": "사탕"},
    "hermione": {"emoji": "🍎", "name": "사과"},
    "voldemort": {"emoji": "🐟", "name": "생선"},
    "dementor": {"emoji": "🍧", "name": "빙수"},
    "hagrid": {"emoji": "🍉", "name": "수박"},
}


def roll_spin(walk_speed):
    """구르기 속도에서 미끄러짐 없는 회전 속도 (deg/s)."""
    return round(walk_speed * ROLL_SPEED_MULT / ROLL_RADIUS * 180 / math.pi, 1)


def manifest(name, walk_speed, frame_counts, speech=None, favorite_food=None, stages=None, stage_size=None, frame_sizes=None):
    """frame_counts: {동작: 프레임 수} — 선택 동작은 들어 있을 때만 포함.
    행 번호: 필수 ANIMS 순서, 그 뒤 frame_counts에 있는 OPTIONAL_ANIMS 순서. speech: 말풍선 대사 (없으면 DEFAULT_SPEECH).
    favorite_food: {"emoji", "name"} · stages: sheet_<stage>.png가 있는 단계 목록 ·
    stage_size: {단계: 종 배율} — 공통 단계 배율(STAGE_SCALE)에 곱한다 (셋 다 있을 때만 적는다) ·
    frame_sizes: {단계: 칸 px} — FRAME과 다른 단계만 (없는 단계는 frame_size)."""
    animations = {}
    rows = list(ANIMS) + [o for o in OPTIONAL_ANIMS if o[0] in frame_counts]
    for row, (anim, _, fps, loop, front) in enumerate(rows):
        spec = {"row": row, "frames": frame_counts[anim], "fps": fps}
        if not loop:
            spec["loop"] = False
        if front:
            spec["front"] = True
        if anim == "roll":
            spec["spin"] = roll_spin(walk_speed)
        animations[anim] = spec
    m = {"name": name, "frame_size": FRAME}
    if frame_sizes:
        m["frame_sizes"] = frame_sizes
    m.update({"walk_speed": walk_speed, "animations": animations, "speech": speech or DEFAULT_SPEECH})
    if favorite_food:
        m["favorite_food"] = favorite_food
    if stages:
        m["stages"] = list(stages)
    if stage_size:
        m["stage_size"] = {st: stage_size[st] for st in (stages or STAGES)}
    return m
