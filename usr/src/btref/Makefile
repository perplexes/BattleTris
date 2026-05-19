#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 18Oct94
#    DESCR: Makefile for BattleTris Referee software
# MODIFIED: 26Oct95
#

include ../Makeinclude

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS) $(NET_LIBS)
CXXFLAGS= $(BT_CXXFLAGS)

EXEC	= btref
OBJS	= btref.o btcmds.o btcmdtab.o btglob.o
LIBS	= -lBTDbase -lBTStdLib -lBTSignals -lm

DSTEXEC	= $(DSTBINDIR)/$(EXEC)

all:	$(EXEC)

$(EXEC): $(OBJS)
	$(CXX) $(CXXFLAGS) -o $(EXEC) $(OBJS) $(IFLAGS) $(LDFLAGS) $(LIBS)

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(EXEC) $(OBJS) core

$(DSTEXEC):	$(EXEC)
	$(INSTALL) -m 0555 $(EXEC) $@

install: $(DSTEXEC)
