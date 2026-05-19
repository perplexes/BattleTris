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

$(LIBRARY): $(LIBOBJ)
	$(AR) $@ $?
	$(RANLIB) $@

.C.o:
	$(CXX) $(CXXFLAGS) $(IFLAGS) -c $<

clean:
	$(RM) $(LIBRARY) $(LIBOBJ) core

install: $(DSTLIB) $(DSTINC)
