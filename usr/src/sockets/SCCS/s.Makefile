h30610
s 00000/00000/00000
d R 1.2 01/10/20 13:35:00 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/Makefile
c Name history : 1 0 src/sockets/Makefile
e
s 00047/00000/00000
d D 1.1 01/10/20 13:34:59 bmc 1 0
c date and time created 01/10/20 13:34:59 by bmc
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
#    DESCR: Makefile for BattleTris sockets library
# MODIFIED: 26Oct95
#

include ../Makeinclude

LIBNAME	= BTSockets
LIBRARY	= lib$(LIBNAME).a

LIBOBJ	= Address.o Socket.o PacketBuffer.o XtSocketCB.o \
	StreamSocketErr.o StreamSocket.o
LIBINC	= Address.H Socket.H PacketBuffer.H SocketCB.H XtSocketCB.H \
	StreamSocket.H StreamSocketErr.H

DSTLIB	= $(DSTLIBDIR)/$(LIBRARY)
DSTINC	= $(DSTINCDIR)/Address.H $(DSTINCDIR)/Socket.H \
	$(DSTINCDIR)/PacketBuffer.H $(DSTINCDIR)/SocketCB.H \
	$(DSTINCDIR)/XtSocketCB.H $(DSTINCDIR)/StreamSocket.H \
	$(DSTINCDIR)/StreamSocketErr.H

IFLAGS	= $(BT_IFLAGS)
LDFLAGS	= $(BT_LDFLAGS) $(X11_LIBS) $(NET_LIBS)
CXXFLAGS= $(BT_CXXFLAGS) $(X11_CFLAGS)

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
