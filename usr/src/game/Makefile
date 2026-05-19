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
	BTFallbacks.o

LIBS	= -lBTAudio -lBTSignals -lBTSockets -lBTStdLib -lBTDbase -lBTWidget -lm

MKDEFS  = -DMKIFLAGS="\"$(IFLAGS)\"" -DMKLDFLAGS="\"$(LDFLAGS)\"" \
	-DMKCXXFLAGS="\"$(CXXFLAGS)\"" -DMKLIBS="\"$(LIBS)\""

DSTEXEC	= $(RELBINDIR)/$(EXEC)
DSTINC	= $(DSTINCDIR)/BTConstants.H $(DSTINCDIR)/BTDebug.H \
	$(DSTINCDIR)/BTDirs.H $(DSTINCDIR)/BTProtocol.H \
	$(DSTINCDIR)/BattleTris.H

all: common $(EXEC)

common: $(DSTINC)

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
