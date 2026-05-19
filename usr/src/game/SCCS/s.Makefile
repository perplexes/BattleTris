h09987
s 00001/00001/00056
d D 1.2 01/10/21 01:52:49 bmc 3 1
c 1000007 audio relies on broken, unshipped header files, libraries
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:33 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/game/Makefile
c Name history : 1 0 src/game/Makefile
e
s 00057/00000/00000
d D 1.1 01/10/20 13:35:32 bmc 1 0
c date and time created 01/10/20 13:35:32 by bmc
e
u
U
f e 0
t
T
I 1
#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 12Oct94
#    DESCR: Makefile for BattleTris executable
# MODIFIED: 4Nov95
#

include ../Makeinclude

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS) $(MOTIF_LIBS) $(X11_LIBS) $(AUDIO_LIBS) $(NET_LIBS)
CXXFLAGS= $(BT_CXXFLAGS) $(MOTIF_CFLAGS) $(X11_CFLAGS) $(AUDIO_CFLAGS)

EXEC	= BattleTris
OBJS	= \
	BTCBoard.o BTStack.o BTBox.o BTRingNode.o \
	BTPiece.o BTPieceManager.o BTArsenal.o BTBoard.o BTScore.o \
	BTBoardManager.o BTSoundManager.o BTScoreManager.o BTBiff.o \
	BTStartup.o BTCommManager.o BTWeaponManager.o BTRecon.o BTGame.o \
	BTChallenge.o BTChallengeDialog.o BTAbout.o BTNetManager.o \
	BTRoster.o BTBazaar.o BTPimp.o BTComputer.o PPMReader.o BattleTris.o \
D 3
	BTArmor.o BTFallbacks.o
E 3
I 3
	BTFallbacks.o
E 3

LIBS	= -lBTAudio -lBTSignals -lBTSockets -lBTStdLib -lBTDbase -lBTWidget -lm

MKDEFS  = -DMKIFLAGS="\"$(IFLAGS)\"" -DMKLDFLAGS="\"$(LDFLAGS)\"" \
	-DMKCXXFLAGS="\"$(CXXFLAGS)\"" -DMKLIBS="\"$(LIBS)\""

DSTEXEC	= $(RELBINDIR)/$(EXEC)
DSTINC	= $(DSTINCDIR)/BTConstants.H $(DSTINCDIR)/BTDebug.H \
	$(DSTINCDIR)/BTDirs.H $(DSTINCDIR)/BTProtocol.H \
	$(DSTINCDIR)/BattleTris.H

all: common $(EXEC)

common: $(DSTINC)

$(DSTINC): $$(@F)
	$(INSTALL) -m 0444 $(@F) $@

$(EXEC): $(OBJS)
	$(CXX) $(CXXFLAGS) -o $@ $(OBJS) $(IFLAGS) $(LIBS) $(LDFLAGS)

.C.o:
	$(CXX) $(CXXFLAGS) $(MKDEFS) $(IFLAGS) -c $<

%.o: %.s
	as -o $@ -P -D_ASM -D__STDC__ $<

clean:
	$(RM) $(EXEC) $(OBJS) core Templates.DB

$(DSTEXEC): $(EXEC)
	$(INSTALL) -m 0555 $(EXEC) $@

install: $(DSTEXEC)
E 1
