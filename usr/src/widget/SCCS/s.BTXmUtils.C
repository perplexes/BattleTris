h21415
s 00027/00021/00013
d D 1.2 01/10/20 15:24:08 bmc 3 1
c 1000004 xm_strcreate needs to take const strings and make a copy
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:21 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTXmUtils.C
c Name history : 1 0 src/widget/BTXmUtils.C
e
s 00034/00000/00000
d D 1.1 01/10/20 13:35:20 bmc 1 0
c date and time created 01/10/20 13:35:20 by bmc
e
u
U
f e 0
t
T
I 1
#include "BTConfig.H"
#include "BTXmUtils.H"

D 3
XmString xm_strcat(XmString s1, XmString s2)
E 3
I 3
#include <alloca.h>

XmString
xm_strcat(XmString s1, XmString s2)
E 3
{
D 3
  register XmString str = XmStringConcat(s1, s2);
E 3
I 3
	XmString str = XmStringConcat(s1, s2);
E 3

D 3
  if(s1)
    XmStringFree(s1);
E 3
I 3
	if (s1)
    		XmStringFree(s1);
E 3

D 3
  return str;
E 3
I 3
	return (str);
E 3
}

D 3
XmString xm_strcreate(char *buf)
E 3
I 3
XmString
xm_strcreate(const char *buf)
E 3
{
D 3
  register XmString str = (XmString) NULL, tmp;
  register char *bufptr;
E 3
I 3
	XmString eoln = XmStringSeparatorCreate();
	XmString str = (XmString)NULL, tmp;
	char *p, *s;
E 3

D 3
  XmString eoln = XmStringSeparatorCreate();
E 3
I 3
	s = (char *) alloca(strlen(buf) + 1);
	(void) strcpy(s, buf);
E 3

D 3
  for(bufptr = strtok(buf, "\n"); bufptr; bufptr = strtok(NULL, "\n")) {
    if(str == (XmString) NULL) {
      str = XmStringCreateLtoR(bufptr, XmFONTLIST_DEFAULT_TAG);
    } else {
      str = xm_strcat(str, eoln);
      tmp = XmStringCreateLtoR(bufptr, XmFONTLIST_DEFAULT_TAG);
      str = xm_strcat(str, tmp);
      XmStringFree(tmp);
    }
  }
E 3
I 3
	for (p = strtok(s, "\n"); p != NULL; p = strtok(NULL, "\n")) {
		if (str == (XmString) NULL) {
			str = XmStringCreateLtoR(p, XmFONTLIST_DEFAULT_TAG);
		} else {
			str = xm_strcat(str, eoln);
			tmp = XmStringCreateLtoR(p, XmFONTLIST_DEFAULT_TAG);
			str = xm_strcat(str, tmp);
			XmStringFree(tmp);
		}
	}
E 3

D 3
  XmStringFree(eoln);
  return str;
E 3
I 3
	XmStringFree(eoln);
	return (str);
E 3
}
E 1
