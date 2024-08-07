#![cfg(test)]

pub(crate) const EXCHANGE_BEHAVIOR_1_UNIT: &str = "\
    DSC22y1Z49l21IhQFh0oJ9l64TPfet44myv4377DXE0xACL43XfsVo13Q2e52uEK\
    v80XNctN4RLH2q3jfPpS2AEMU31gVJcw0JF1R03moTTo2DIJVW4VdGXN4DfvLt2J\
    Ji4x4LJQ2g2FglIy0adSA01jc2zu0VW7C52BuTh54RIo2s4dRP9027hoCf2g8gTR\
    4PDRnB2UeSwR26Sc3g4OsXKO3Sr04Y2hwMdg3AM1Sp0p2PHD2fo2tS3MDgqb3dpy\
    Le1gEH3y1ylKwg0HIFq91T8ONE0VcdXW3aIloJ2AH5324B5lWI25PEEV1aH4iP2k\
    NlBr3JSx3J0gFGx403B8xo2NDi0V25KKwQ0fj0xL39fMwO0fbCA01PKbYP3Cu57P\
    3pfZvK1x0M6z0xM1t90XCfBZ3FkvAH4GcVxw1RFYsn4eZAyj2idbiS3ps71P1gPs\
    Vd0CkS3Z23XL7T4MdoqZ2ymqOz0fdGIx2Q0rcR38K7pC10KdXu2TJ5f33gWjlj1y\
    pMDd3QlzdM3YdoW11U1hoB2l2U7T2P2T8W4ctY0a0Pcqe60WVSV31BowIl0h46Zd\
    1ME5sj2EppSX3toTlN2Rmtdi4XVV6O4arVHS3ILZia1oMpXw0tpPnE1VZuLe0IGC\
    112CCAVe3NIyZc1tABRc1YzBmu2Wt76c41Dsrq15A0kF0F1qC34Zjwdx0Ul3Og0i\
    vM2Z1nOXbO352YXD0roDDA2hTmk83tzqyF43w76T1Art1M4CE7qL0RnpOJ0e45E6\
    2YrOfd2hEeb510mhbc4TSYua41sEVu2eEQ9C1nLKHf475iAV3SFX153ENfFH1kfA\
    GJ1F1hd21laEpw4SCS8v2lHys03u1EYv1mK1f62z9Z3q20npE92OSB2v0oFLuj1c\
    96Nt1h0vTK0t1Tu62t4z7v0rTQ7C3UTyEN3Vicqb1j5msz0mjxqe2SaKQD2MavcV\
    2XBkFp2ScU1o4SiGUy0CZcjB1xVbdw0AfZzb0RetOD1xy49p354hT743hvqM4c4i\
    1Y3BBXhh0WEJxw27QirN32riX70giDyM21fYvC1jBtyT4KXout2F0sVD1beemU23\
    vycT1gw9ng4770z042l8pe2uLzoa2B4bKn2SHcSi3RU27V1kRten2lCrYF3o8Saz\
    242QpN0EkQ8a2r7HS03mjw9k3tESSx22g0600iHhKx1E0j9A4JfXld1GcaOJ2UiR\
    l740la5g0cx9mn0oe0eK3o8Vbj39qK2k0oun7F29ii4v275I3a02Pa9T04gPAZ\
";

pub(crate) const EXCHANGE_BEHAVIOR_2: &str = "\
    DSC2Az1Z49l210ZIJZ1CGTxo2wnGzt1BSpuq4TxlWR4ACY2C08sw230BCpOy0JSk\
    Xe2gAv5m0dPrZr4023vV3g84wB1L8ajW0Tm0wl1Npiyh3maDqZ3hYOFm2LcKI64D\
    bHHx1Hw01z4BzGgB2NmB0b2sLX0h10C6di3zg3UR4VqG9i2PlkPg12He480BSkI5\
    473GsK0ph4iu1gCxcA4VgUUG2UNttG2iaR4B0lXdY81gkBA81zx3te1MO0Yp4Iov\
    iA1vdCnx0PaTme4XPVBL2ExbH70Dy5lI2k2btG3gG6jl0ZCPOn2aSJy40hd3ui1Z\
    kfng4cjk7l3WR7xE40HAYV0XYT991WSqH10pKi432BwEHB3kVXQM2xnzOU3LGHwn\
    3HSYeN1A6ony3SHHA94D4MzS3drZRP1DxXb23JJyY32xyLoH2DFCMp4YWOJ83uNd\
    sh1jejhM42BBgd0jDxxo4YeG923ZqzJc0sDaoo23dwtd4eL5tF0NS6ZQ3k4Hvq0b\
    Uf2W3oUTvl3RM7132JOEBF0dKVt73VVQ8d3Us6rS4T87RG2IqDAm0Xmvaz4DP5Fk\
    0aOxxS0PtqRt2UBGYE47qFQ81zXnrk0YqewJ3kYvgh29JDhF2Jw0Sx0Se1uh3WPi\
    Sr4RhfOJ3UEd200w1zBS0C8TU22BrJKC2D3Wnj0G7egW3NdC340Gn8Kj34QMy633\
    r9mp3t6ujL21lvIt1tPV0i3Z0nod0CvLSu0fwA880Os15W2ZKR0T0V1XR13eEh6s\
    07ozo70VjOHS13oOQD2aj07T3ldSIg1MiXl91jgEhl1qkkcr48Hcbt3UZT7g0UOF\
    GV1ih3KG090Omk11v3od2tC4qc3pVyWU3O67cp3eXg8C2HhVC00yrtyL2TziTx3A\
    fuWH4YbvLl0sAjnL0HQYHx3hyxe83nsCas2TvfcM3mtpjT34CSYQ23y6ef1twU8M\
    2WU8pS4cJehP1QQTBh0INdJp2n1w8t34kxxY2QJPJQ3vFLNj3H09M70Alkhq09g2\
    Fk0bs2Dc2KiPCN0p7ENr4QtkXh2Fg8Su2dIEYY0qhmhY12xLPI0s0VWO3Unc4Y2E\
    i3kJ2bduN32Ziol315CBfx0rEsZz41Gx0M4Mtf7219RwBm3HhJJT1mGXWU3tWVAp\
    2CIgWR3Lob1P4V7B624bbP2F1vTkKv0dEWJ20bTB824Zfkp53sBWeJ3y6OCM10t0\
    aN1aSZv12vVVGl3eTJC80oA4PW128q5C23Zz6I2OpVLZ062fbo2bFVWJ1y4aYO49\
    rCGq1ycHV945ATyQ3DzEd03HVOe83w58N63jaCJ10dmnUd2c67ut0ZbgY22TigI6\
    1UPsIE22FMNV2ZHhJv4SLJLc0RsMrl2Da6NL30aunz2fiPnH1TLRMC0oXgvu2dRc\
    Ol08c9zx2qXm1q2YB2s13zJyBn34tEeN0CCxJi\
";

pub(crate) const EXCHANGE_BEHAVIOR_3_PARAM: &str = "\
    DSC22s2jsGca1BbRzp1aSXx241qtzU2XKfaj46zjp42c2xSy1GaHRJ1U1ve120SO\
    ft0eW4qw1DBX2Z1JBJHl0Ij8q00DWHeX2UwwJ23v0ZSu1nm47i3H4yyp4ZPxMJ1t\
    7Itz2jQta72DnXDU3U9rfF1JTdIh2bpWaK3CZ0uH4H1PFq363mNf1dpntN2l3Sir\
    15gOjz0dPTo51eGasT2awBMA0ei3Bi2xeASi0nTEdN4JrtKW1glD6i3L8MFJ3vp6\
    LB3vKWW01uQEli2GTBvU0YHoQm2xU0O021YHxX3zAsPe3iOJwQ3XEYgj2TjZEM1z\
    yJog33S1Mx2iB4EN38trmH4WsLeS08UlrR32Bl1J1IZ2JV1HyRJz1HtALf2cslgL\
    2azh0x4Zvd7u1HqCrJ3HJJL42OO2ut0NFFYu43YTm63OaGjb2QyShd1ce3ob3GSQ\
    lI0KbAUR3QQNg93kAgia1OtIhN2BCyKh49Zv1u1Nbroq2VETHI29PvtT3XmD2f3y\
    lCTR32VlfD0Q7X8h4KfRGK42iRSI1WXE382SqMNx4EtiNy3h7x1s2NhaNm122sGo\
    3h2kPu0rmBy42OEoe34GIyWw2JDyRG2kNwjb3lpwDN42H4Am0w2sNX0eLfpV0XT4\
    3k3iHYCn0EnTQm2dssUc3cbWLA1mFUqS14ixl02z7Ycw1bzH9i3RiQ201C2VK809\
    4WDD31qIrm1Kfsn632tBe13FIEbd0LMzdQ3NYPC62OBMvM0hd8B73XFxqn2VqWKv\
    0Tsv4q1hzam30kFZOH1Lcr4f23tx6z2032Rt2ROghI0waEMd12vTdO3FPop10YO8\
    NU3zQGvu2f1vOg4BbjZv3LsJUK0XTix249HGtT3YyGg04RCXfK0cPVVK0TTxMu3k\
    2Mqd2uJsvK1lgL4R2p6HjI3pMzr01FbDGe0RjXCn0xAazC3ZJrNZ2mr0nT1RpkGL\
    1pbtOE4JKTzi1xMvMl4KkQ633VeFUH4Kk6Yo3EyYAV0LNnvc0vvvfB09g0230jin\
    vD1lkWui4bXMcz0fcVXC38zDmM0M2sLb4A3QuN40llgn4Qgoj647cNRz1beXHN2t\
    b4sH2kZSIe0EmnsD02KUeV3uIkyj2KUUFN3WJjDV1uj\
";

pub(crate) const EXCHANGE_BEHAVIOR_4_SUB: &str = "\
    DSCBp1S9BSi1BX0oT2lvaIy2Trdq631uFLy2U7Msb2SnGmL2qlraI1KXpLp2OimE\
    w0Jbls03yhU3L2KwYIE0TU5ey2LzPJe1stI6l4UQBXa1JZ0ZM06rXJs13VdoZ4cg\
    GcC3YCGvq4KEF2N2nWXeS2Vwz2x1UJS0q1uIloQ1Zgkg2215oH344HzIX2ajWkG1\
    rJtYH2afngS3g28UT2AO6mD241iyI36CkL234Md9t0fordD31jb3k4BCe8b3MmCU\
    K1uRpAR07kiJW04RXjE3MA6Vi2DA5na0jWBGK3sI58Z2ZSaBl3nC77K3kvW6l2LF\
    8ll454jQu14c1Xt4fxLMZ0XysbO0kq1hG2Iv7oi0FL9NH0jMiid1fNByA4ZoPLB1\
    Hxvbh2TXeff0jq\
";

pub(crate) const RON_VALUE_1: &str = r#"{
    "bool1"  : true ,
    "bool2"  : false,
    "int1"   :   42,
    "int2"   :  -42,
    "int3"   :    0,
    "float1" :  42.0,
    "float2" : -42.0,
    "float3" :   0.0,
    "string" : "string",
    "array" : [1, 2, 3, 4],
    "array_but_different" : {1: 1, 2: 2, 3: 3, 4: {}, 5: {}},
    "map" : {"key" : "value"},
    "mixed_table" : {1: 42, -1: -42, 0: 0, "key" : "value"},
}"#;

pub(crate) const RON_VALUE_1_COMPACT: &str = const_format::concatcp!(
    "{",
    r#""array":[1,2,3,4],"#,
    r#""array_but_different":[1,2,3,{},{}],"#,
    r#""bool1":true,"bool2":false,"#,
    r#""float1":42.0,"float2":-42.0,"float3":0.0,"#,
    r#""int1":42,"int2":-42,"int3":0,"#,
    r#""map":{"key":"value"},"#,
    r#""mixed_table":{-1:-42,0:0,1:42,"key":"value"},"#,
    r#""string":"string""#,
    "}"
);

