program fib_demo
    implicit none
    integer :: n
    character(len=16) :: argument
    call get_command_argument(1, argument)
    read(argument, *) n
    print *, fib(n)
contains
    recursive integer(8) function fib(n) result(answer)
        integer, intent(in) :: n
        if (n < 2) then
            answer = n
        else
            answer = fib(n - 1) + fib(n - 2)
        end if
    end function fib
end program fib_demo
