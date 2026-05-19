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
