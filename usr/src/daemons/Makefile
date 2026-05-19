#
#     NAME: Makefile
#   AUTHOR: Michael Shapiro
#     DATE: 27Sep94
#    DESCR: Makefile for BattleTris Server Daemon and Slave Daemon
# MODIFIED: 26Oct95
#

include ../Makeinclude

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS) $(MOTIF_LIBS) $(X11_LIBS) $(NET_LIBS)
CXXFLAGS= $(BT_CXXFLAGS) $(MOTIF_CFLAGS) $(X11_CFLAGS)

EXEC1	= btserverd
OBJS1	= BTMDSlave.o BTServer.o btserverd.o

EXEC2	= btslaved
OBJS2	= BTSDClient.o BTDBServer.o BTSlave.o btslaved.o

LIBS	= -lBTDbase -lBTSockets -lBTStdLib -lBTSignals

DSTEXEC1= $(DSTBINDIR)/$(EXEC1)
DSTEXEC2= $(DSTBINDIR)/$(EXEC2)

all:	$(EXEC1) $(EXEC2)

$(EXEC1): $(OBJS1)
	$(CXX) $(CXXFLAGS) -o $(EXEC1) $(OBJS1) $(IFLAGS) $(LDFLAGS) $(LIBS)

$(EXEC2): $(OBJS2)
	$(CXX) $(CXXFLAGS) -o $(EXEC2) $(OBJS2) $(IFLAGS) $(LDFLAGS) $(LIBS)

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(EXEC1) $(EXEC2) $(OBJS1) $(OBJS2) Templates.DB core

$(DSTEXEC1):	$(EXEC1)
	$(INSTALL) -m 0555 $(EXEC1) $@

$(DSTEXEC2):	$(EXEC2)
	$(INSTALL) -m 0555 $(EXEC2) $@

install: $(DSTEXEC1) $(DSTEXEC2)
