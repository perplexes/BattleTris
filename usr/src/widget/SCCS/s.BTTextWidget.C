h15926
s 00023/00023/00020
d D 1.2 01/10/21 19:25:18 bmc 3 1
c 1000011 compile game, widget with no warnings (anachronisms remain)
e
s 00000/00000/00000
d R 1.2 01/10/20 13:35:17 Codemgr 2 1
c SunPro Code Manager data about conflicts, renames, etc...
c Name history : 2 1 usr/src/widget/BTTextWidget.C
c Name history : 1 0 src/widget/BTTextWidget.C
e
s 00043/00000/00000
d D 1.1 01/10/20 13:35:16 bmc 1 0
c date and time created 01/10/20 13:35:16 by bmc
e
u
U
f e 0
t
T
I 1
/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTTextWidget.C                                      */
/*    ASSN:                                                     */
/*    DATE: Thu Apr 28 23:20:15 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTTextWidget.H"

D 3
BTTextWidget::BTTextWidget(BTWidget *parent, char *const name,
                           Position x1, Position y1, Position x2, Position y2,
                           short rows, short cols) 
E 3
I 3
BTTextWidget::BTTextWidget(BTWidget *parent, const char *name,
    Position x1, Position y1, Position x2, Position y2, short rows, short cols)
E 3
: BTWidget(parent), rows_(rows), cols_(cols), tab_(2)
{
D 3
  char *buf = new char [rows * cols];
  register short r, i;
E 3
I 3
	char *buf = new char [rows * cols];
	register short r, i;
E 3

D 3
  for(r = 0; r < rows; r++) {
    for(i = 0; i < cols - 1; i++) 
      buf[r * cols + i] = ' ';
    buf[r * cols + i] = '\n';
  }
E 3
I 3
	for (r = 0; r < rows; r++) {
 		for (i = 0; i < cols - 1; i++) 
			buf[r * cols + i] = ' ';
		buf[r * cols + i] = '\n';
	}
E 3
    
D 3
  me_ =
    XtVaCreateWidget(name, xmTextWidgetClass, parent ? parent->getWidget() : 0,
                     XmNrows, rows, XmNcolumns, 80,
                     XmNeditMode, XmMULTI_LINE_EDIT, XmNeditable, False,
                     XmNautoShowCursorPosition, False,
                     XmNcursorPositionVisible, False,
                     XmNx, x1, XmNy, y1,
                     XmNwidth, x2 - x1 + 1, XmNheight, y2 - y1 + 1,
                     XmNvalue, buf, NULL);
E 3
I 3
	me_ = XtVaCreateWidget(name, xmTextWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNrows, rows, XmNcolumns, 80,
	    XmNeditMode, XmMULTI_LINE_EDIT, XmNeditable, False,
	    XmNautoShowCursorPosition, False,
	    XmNcursorPositionVisible, False,
	    XmNx, x1, XmNy, y1,
	    XmNwidth, x2 - x1 + 1, XmNheight, y2 - y1 + 1,
	    XmNvalue, buf, NULL);
E 3

D 3
  delete [] buf;
E 3
I 3
	delete [] buf;
E 3
}

D 3
void BTTextWidget::setText(short row, short tab_stop, char *text)
E 3
I 3
void
BTTextWidget::setText(short row, short tab_stop, const char *text)
E 3
{
D 3
  XmTextReplace(me_, row * cols_ + tab_stop * tab_, 
		row * cols_ + tab_stop * tab_ + strlen(text), text);
E 3
I 3
	XmTextReplace(me_, row * cols_ + tab_stop * tab_, 
	    row * cols_ + tab_stop * tab_ + strlen(text), (char *)text);
E 3
}
E 1
