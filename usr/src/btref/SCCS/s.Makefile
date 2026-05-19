h04711
s 00000/00000/00000
d R 1.2 01/10/20 13:34:53 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/btref/Makefile
c Name history : 1 0 src/btref/Makefile
e
s 00035/00000/00000
d D 1.1 01/10/20 13:34:52 bmc 1 0
c CodeManager Uniquification: src/btref/Makefile
c date and time created 01/10/20 13:34:52 by bmc
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
E 1
