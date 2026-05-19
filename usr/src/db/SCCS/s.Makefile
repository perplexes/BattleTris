h48552
s 00000/00000/00000
d R 1.2 01/10/20 13:34:49 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/db/Makefile
c Name history : 1 0 src/db/Makefile
e
s 00052/00000/00000
d D 1.1 01/10/20 13:34:48 bmc 1 0
c date and time created 01/10/20 13:34:48 by bmc
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
#     DATE: 27Apr94
#    DESCR: Makefile for BattleTris database library
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTDbase
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= BTDB.o BTDBLock.o BTDBRecord.o BTDBErr.o BTNetworkEntry.o \
	BTPlayer.o BTPlayerRecord.o BTGameStats.o BTConfigFile.o ParsedFile.o

LIBINC	= BTDB.H BTDBLock.H BTDBReadLock.H BTDBWriteLock.H BTDBRecord.H	\
	BTDBErr.H BTNetwork.H BTNetworkEntry.H BTPlayer.H BTPlayerRecord.H \
	BTGameStats.H BTConfigFile.H ParsedFile.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/BTDB.H $(DSTINCDIR)/BTDBLock.H \
	$(DSTINCDIR)/BTDBReadLock.H $(DSTINCDIR)/BTDBWriteLock.H \
	$(DSTINCDIR)/BTDBRecord.H $(DSTINCDIR)/BTDBErr.H \
	$(DSTINCDIR)/BTNetwork.H $(DSTINCDIR)/BTNetworkEntry.H \
	$(DSTINCDIR)/BTPlayer.H $(DSTINCDIR)/BTPlayerRecord.H \
	$(DSTINCDIR)/BTGameStats.H $(DSTINCDIR)/BTConfigFile.H \
	$(DSTINCDIR)/ParsedFile.H

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS)
CXXFLAGS= $(BT_CXXFLAGS)

all:	$(LIBRARY)

$(DSTLIB): $(LIBRARY)
	$(INSTALL) -m 0444 $(LIBRARY) $@

$(DSTINC): $$(@F)
	$(INSTALL) -m 0444 $(@F) $@

$(LIBRARY): $(LIBOBJ)
	$(AR) $@ $?
	$(RANLIB) $@

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(LIBRARY) $(LIBOBJ) core

install: $(DSTLIB) $(DSTINC)
E 1
