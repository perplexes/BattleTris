/****************************************************************/
/*    NAME: Bryan Cantrill                                      */
/*    ACCT: bmc                                                 */
/*    FILE: BTTextWidget.C                                      */
/*    ASSN:                                                     */
/*    DATE: Thu Apr 28 23:20:15 1994                            */
/****************************************************************/

#include "BTConfig.H"
#include "BTTextWidget.H"

BTTextWidget::BTTextWidget(BTWidget *parent, const char *name,
    Position x1, Position y1, Position x2, Position y2, short rows, short cols)
: BTWidget(parent), rows_(rows), cols_(cols), tab_(2)
{
	char *buf = new char [rows * cols];
	register short r, i;

	for (r = 0; r < rows; r++) {
 		for (i = 0; i < cols - 1; i++) 
			buf[r * cols + i] = ' ';
		buf[r * cols + i] = '\n';
	}
    
	me_ = XtVaCreateWidget(name, xmTextWidgetClass,
	    parent ? parent->getWidget() : 0,
	    XmNrows, rows, XmNcolumns, 80,
	    XmNeditMode, XmMULTI_LINE_EDIT, XmNeditable, False,
	    XmNautoShowCursorPosition, False,
	    XmNcursorPositionVisible, False,
	    XmNx, x1, XmNy, y1,
	    XmNwidth, x2 - x1 + 1, XmNheight, y2 - y1 + 1,
	    XmNvalue, buf, NULL);

	delete [] buf;
}

void
BTTextWidget::setText(short row, short tab_stop, const char *text)
{
	XmTextReplace(me_, row * cols_ + tab_stop * tab_, 
	    row * cols_ + tab_stop * tab_ + strlen(text), (char *)text);
}
