h49413
s 00001/00001/00008
d D 1.2 01/10/20 15:24:09 bmc 3 1
c 1000004 xm_strcreate needs to take const strings and make a copy
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:19 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTXmUtils.H
c Name history : 1 0 src/widget/BTXmUtils.H
e
s 00009/00000/00000
d D 1.1 01/10/20 13:35:18 bmc 1 0
c date and time created 01/10/20 13:35:18 by bmc
e
u
U
f e 0
t
T
I 1
#ifndef BTXMUTILS_H
#define BTXMUTILS_H

#include <Xm/Xm.h>

XmString xm_strcat(XmString s1, XmString s2);
D 3
XmString xm_strcreate(char *buf);
E 3
I 3
XmString xm_strcreate(const char *buf);
E 3

#endif
E 1
