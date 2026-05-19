h54060
s 00000/00000/00000
d R 1.2 01/10/20 13:35:02 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/sockets/PacketBuffer.H
c Name history : 1 0 src/sockets/PacketBuffer.H
e
s 00054/00000/00000
d D 1.1 01/10/20 13:35:01 bmc 1 0
c date and time created 01/10/20 13:35:01 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Michael Shapiro                                     */
/*    ACCT: mws                                                 */
/*    FILE: PacketBuffer.H                                      */
/*    DATE: Thu Dec 22 23:16:27 1994                            */
/****************************************************************/

#ifndef PACKETBUFFER_H
#define PACKETBUFFER_H

#include "Socket.H"

typedef unsigned short netshort;
typedef unsigned long netlong;

class PacketBuffer {
public:
  enum {
    PB_FIXEDBUFLEN = 8192
  };

protected:
  char fixedbuf_[PB_FIXEDBUFLEN];
  char *dynabuf_;
  netlong header_[2];
  Socket *sock_;
  char *bufptr_;
  long buflen_;
  
public:
  PacketBuffer(Socket *sock = 0)
  : dynabuf_(0), sock_(sock), bufptr_(fixedbuf_), buflen_(PB_FIXEDBUFLEN) {}

  ~PacketBuffer() {
    if(dynabuf_)
      delete [] dynabuf_;
  }

  short sendpacket(netlong type, netlong nbytes = 0, char *data = 0);
  short recvpacket();
  short peekpacket();

  netlong datatype() { return header_[0]; }
  netlong datalen() { return header_[1]; }
  char *databuf() { return bufptr_; }

  void socket(Socket *sock) { sock_ = sock; }

private:
  PacketBuffer(const PacketBuffer& other);
  PacketBuffer& operator=(const PacketBuffer& other);
};

#endif
E 1
